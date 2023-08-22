use crate::error::{InternalPaymentError, ParametersError};
use crate::icrc1::{self, TokenTransferInfo};
use crate::{Timestamp, TokenConfiguration};
use candid::{CandidType, Deserialize, Nat, Principal};
use ic_exports::ic_kit::ic;
use ic_exports::icrc_types::icrc1::account::{Account, Subaccount};
use ic_exports::icrc_types::icrc1::transfer::Memo;

/// Transfer to be executed.
#[derive(Debug, CandidType, Deserialize, Clone)]
pub struct Transfer {
    /// Token principal.
    pub token: Principal,

    /// Initiator of the transfer. This principal's balance will be used for balance operation (if
    /// any).
    pub caller: Principal,

    /// Subaccount to transfer from.
    pub from: Option<Subaccount>,

    /// Account to transfer to.
    pub to: Account,

    /// Amount to transfer. This amount includes the fee, so the actual value that will be received
    /// by the `to` account is `amount - fee`.
    pub amount: Nat,

    /// Transaction fee.
    pub fee: Nat,

    /// Operation to execute after the transfer finished.
    pub operation: Operation,

    /// Type of the transfer.
    pub r#type: TransferType,

    /// Timestamp when the transaction was created. This timestamp is used for the transaction
    /// deduplicated.
    pub created_at: Timestamp,

    /// Arbitrary byte-string that can be added to the transaction. Use this field in case several
    /// transfers with the same timestamp must be done.
    pub memo: Option<Memo>,
}

/// Operation to be executed after the transfer is finished.
#[derive(Debug, Eq, PartialEq, CandidType, Deserialize, Clone, Copy)]
pub enum Operation {
    /// Do nothing.
    None,

    /// Add `amount - fee` to the caller's balance if the transfer is successful.
    CreditOnSuccess,

    /// Add `amount` to the caller's balance if the transfer fails.
    CreditOnError,
}

/// Type of the transfer.
#[derive(Debug, CandidType, Deserialize, Clone)]
pub enum TransferType {
    /// Direct transfer from `from` account to `to` account.
    SingleStep,

    /// Transfer through an interim account, given as the second parameter.
    DoubleStep(Stage, Account),
}

/// Current step of a double-step transfer.
#[derive(Debug, CandidType, Deserialize, Clone, Copy)]
pub enum Stage {
    First,
    Second,
}

const INTERMEDIATE_ACC_DOMAIN: &[u8] = b"is-amm-intermediate-acc";

impl Transfer {
    /// Creates a new transfer.
    ///
    /// This constructor can be chained with other methods like [`Transfer::with_operation`] or [`Transfer::double_step`] to further configure the transfer.
    ///
    /// ```no_run
    /// # use ic_payments::*;
    /// # use ic_exports::icrc1::account::{Account, Subaccount};
    /// # use candid::Principal;
    /// # let token_config = ic_payments::TokenConfiguration { principal:
    /// # Principal::management_canister(), fee: 0.into(), minting_account: Account { owner: Principal::management_canister().into(), subaccount: None }};
    /// # let caller = Principal::management_canister();
    /// # let to = caller.into();
    /// let transfer = Transfer::new(&token_config, caller, to, None, 10_000.into())
    ///     .with_operation(Operation::CreditOnSuccess)
    ///     .double_step();
    /// ```
    pub fn new(
        token_config: &TokenConfiguration,
        caller: Principal,
        to: Account,
        from_subaccount: Option<Subaccount>,
        amount: Nat,
    ) -> Self {
        let fee = token_config.get_fee(
            &Account {
                owner: ic::id(),
                subaccount: from_subaccount,
            },
            &to,
        );
        Self {
            token: token_config.principal,
            caller,
            from: from_subaccount,
            to,
            amount,
            fee,
            operation: Operation::None,
            r#type: TransferType::SingleStep,
            created_at: ic::time(),
            memo: None,
        }
    }

    /// Sets the operation of the transfer to be the given one.
    pub fn with_operation(self, operation: Operation) -> Self {
        Self { operation, ..self }
    }

    /// Makes the transfer double-step.
    pub fn double_step(self) -> Self {
        let interim_acc = match self.r#type {
            TransferType::SingleStep => self.generate_interim_acc(),
            TransferType::DoubleStep(_, interim_acc) => interim_acc,
        };

        Self {
            r#type: TransferType::DoubleStep(Stage::First, interim_acc),
            ..self
        }
    }

    /// Sets memo for transactions of this transfer.
    pub fn with_memo(self, memo: Memo) -> Self {
        Self {
            memo: Some(memo),
            ..self
        }
    }

    /// Executes the transfer.
    ///
    /// This method does not consume the transfer since the caller might need to retry executing it
    /// in case of a transient error.
    pub async fn execute(&self) -> Result<TokenTransferInfo, InternalPaymentError> {
        icrc1::transfer_icrc1(
            self.token,
            self.to(),
            self.amount_minus_fee(),
            self.fee.clone(),
            self.from().subaccount,
            Some(self.created_at()),
            self.memo.clone(),
        )
        .await
    }

    pub(crate) fn id(&self) -> [u8; 32] {
        use sha2::{Digest, Sha224};

        let mut hash = Sha224::default();
        hash.update(INTERMEDIATE_ACC_DOMAIN);
        hash.update(&self.from.unwrap_or_default());
        hash.update(self.to.owner.as_slice());
        hash.update(self.to.effective_subaccount());
        hash.update(&self.amount.0.to_bytes_le());
        hash.update(self.token.as_slice());
        hash.update(&self.created_at.to_le_bytes());

        let hash_result: [u8; 28] = hash.finalize().into();
        let mut subaccount = [0; 32];
        subaccount[0..4].copy_from_slice(b"vfrc");
        subaccount[4..].copy_from_slice(&hash_result);

        subaccount
    }

    pub(crate) fn from(&self) -> Account {
        match &self.r#type {
            TransferType::SingleStep => self.from_acc(),
            TransferType::DoubleStep(Stage::First, _) => self.from_acc(),
            TransferType::DoubleStep(Stage::Second, acc) => *acc,
        }
    }

    /// Source account of the transfer.
    pub fn from_acc(&self) -> Account {
        Account {
            owner: ic::id(),
            subaccount: self.from,
        }
    }

    /// Target account of the transfer.
    pub fn to(&self) -> Account {
        match &self.r#type {
            TransferType::SingleStep => self.to,
            TransferType::DoubleStep(Stage::First, acc) => *acc,
            TransferType::DoubleStep(Stage::Second, _) => self.to,
        }
    }

    /// Interim account of the transfer.
    ///
    /// Returns `None` if the transfer is single-step.
    pub fn interim_acc(&self) -> Option<Account> {
        match &self.r#type {
            TransferType::DoubleStep(_, acc) => Some(*acc),
            _ => None,
        }
    }

    fn generate_interim_acc(&self) -> Account {
        Account {
            owner: ic::id(),
            subaccount: Some(self.id()),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), InternalPaymentError> {
        if self.from_acc() == self.to {
            return Err(InternalPaymentError::InvalidParameters(
                ParametersError::TargetAccountInvalid,
            ));
        }

        if self.final_amount()? == 0 {
            return Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: self.min_amount(),
                    actual: self.amount.clone(),
                },
            ));
        }

        Ok(())
    }

    /// Effective fee of the transfer.
    ///
    /// Effective fee can be different from the value in the token configuration:
    /// 1. If the from or to account of the transfer is the token minting account, the transfer fee
    ///    is set to 0 according to ICRC-1 standard.
    /// 2. If the transfer is double-step, effective fee will be twice the configured amount, since
    ///    the transfer requires two transactions to be completed.
    pub fn effective_fee(&self) -> Nat {
        match self.r#type {
            TransferType::DoubleStep(Stage::First, _) => self.fee.clone() * 2,
            _ => self.fee.clone(),
        }
    }

    fn min_amount(&self) -> Nat {
        self.effective_fee() + 1
    }

    /// Amount to be transferred.
    pub fn amount(&self) -> Nat {
        self.amount.clone()
    }

    pub(crate) fn amount_minus_fee(&self) -> Nat {
        match self.amount > self.fee {
            true => self.amount.clone() - self.fee.clone(),
            false => 0.into(),
        }
    }

    /// Amount that `to` account will receive after the transfer is complete.
    pub fn final_amount(&self) -> Result<Nat, InternalPaymentError> {
        let effective_fee = self.effective_fee();
        match self.amount > effective_fee {
            true => Ok(self.amount.clone() - effective_fee),
            false => Err(ParametersError::AmountTooSmall {
                minimum_required: self.min_amount(),
                actual: self.amount.clone(),
            }
            .into()),
        }
    }

    /// Operation to be executed after the transfer is completed.
    pub fn operation(&self) -> Operation {
        self.operation
    }

    /// Caller of the transfer. This principal will be used for the balance operation.
    pub fn caller(&self) -> Principal {
        self.caller
    }

    /// Updates `created_at` to current time.
    pub fn renew(self) -> Self {
        Self {
            created_at: ic::time(),
            ..self
        }
    }

    /// Updates the fee amount configured for the transfer.
    pub fn with_fee(self, fee: Nat) -> Self {
        Self { fee, ..self }
    }

    /// Sets created_at value to 0. The transfer with such parameter cannot be executed, so this
    /// can be used to force renewal of trnasfers on recovery proces.
    pub fn reset_ts(self) -> Self {
        Self {
            created_at: 0,
            ..self
        }
    }

    /// Timestamp when the transfer was created.
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Type of the transfer.
    pub fn r#type(&self) -> &TransferType {
        &self.r#type
    }

    /// Creates a new transfer which is the second step of a double-step transfer.
    ///
    /// Returns `None` if the transfer is not a first step of a double-step transfer.
    pub fn next_step(&self) -> Option<Self> {
        match &self.r#type {
            TransferType::DoubleStep(Stage::First, interim_acc) => Some(Self {
                r#type: TransferType::DoubleStep(Stage::Second, *interim_acc),
                amount: self.amount_minus_fee(),
                created_at: ic::time(),
                memo: self.memo.clone(),
                fee: self.fee.clone(),
                ..*self
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ic_exports::ic_kit::mock_principals::{alice, bob, john, xtc};
    use ic_exports::ic_kit::MockContext;

    use super::*;
    use crate::error::ParametersError;

    #[test]
    fn validate_single_step_amount() {
        MockContext::new().with_id(john()).inject();
        let mut transfer = Transfer {
            token: alice(),
            caller: bob(),
            from: None,
            to: Account {
                owner: bob(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::SingleStep,
            created_at: 0,
            memo: None,
        };

        assert!(transfer.validate().is_ok());
        transfer.fee = 100.into();
        assert!(transfer.validate().is_ok());
        transfer.fee = 999.into();
        assert!(transfer.validate().is_ok());
        transfer.fee = 1000.into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: 1001.into(),
                    actual: 1000.into()
                }
            ))
        );
        transfer.fee = 10000.into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: 10001.into(),
                    actual: 1000.into()
                }
            ))
        );
        transfer.fee = u128::MAX.into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: transfer.fee + 1,
                    actual: 1000.into(),
                }
            ))
        );
    }

    #[test]
    fn validate_first_stage_amount() {
        MockContext::new().with_id(john()).inject();
        let mut transfer = Transfer {
            token: alice(),
            caller: bob(),
            from: None,
            to: Account {
                owner: bob(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::DoubleStep(
                Stage::First,
                Account {
                    owner: john(),
                    subaccount: Some([1; 32]),
                },
            ),
            created_at: 0,
            memo: None,
        };

        assert!(transfer.validate().is_ok());
        transfer.fee = 100.into();
        assert!(transfer.validate().is_ok());
        transfer.fee = 499.into();
        assert!(transfer.validate().is_ok());
        transfer.fee = 500.into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: 1001.into(),
                    actual: 1000.into()
                }
            ))
        );
        transfer.fee = 10000.into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: 20001.into(),
                    actual: 1000.into()
                }
            ))
        );
        transfer.fee = (u128::MAX / 2).into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: u128::MAX.into(),
                    actual: 1000.into()
                }
            ))
        );
    }

    #[test]
    fn validate_second_stage_amount() {
        MockContext::new().with_id(john()).inject();
        let mut transfer = Transfer {
            token: alice(),
            caller: bob(),
            from: None,
            to: Account {
                owner: bob(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::DoubleStep(
                Stage::Second,
                Account {
                    owner: john(),
                    subaccount: Some([1; 32]),
                },
            ),
            created_at: 0,
            memo: None,
        };

        assert!(transfer.validate().is_ok());
        transfer.fee = 100.into();
        assert!(transfer.validate().is_ok());
        transfer.fee = 999.into();
        assert!(transfer.validate().is_ok());
        transfer.fee = 1000.into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: 1001.into(),
                    actual: 1000.into()
                }
            ))
        );
        transfer.fee = 10000.into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: 10001.into(),
                    actual: 1000.into()
                }
            ))
        );
        transfer.fee = u128::MAX.into();
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: transfer.fee + 1,
                    actual: 1000.into(),
                }
            ))
        );
    }

    #[test]
    fn validate_to_self() {
        MockContext::new().with_id(john()).inject();
        let transfer = Transfer {
            token: alice(),
            caller: bob(),
            from: Some([1; 32]),
            to: Account {
                owner: john(),
                subaccount: Some([1; 32]),
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::SingleStep,
            created_at: 0,
            memo: None,
        };

        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::TargetAccountInvalid
            ))
        );
    }

    fn simple_transfer() -> Transfer {
        Transfer {
            token: alice(),
            caller: bob(),
            from: None,
            to: Account {
                owner: bob(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::SingleStep,
            created_at: 0,
            memo: None,
        }
    }

    #[test]
    fn id_unique_over_recipient() {
        let t1 = simple_transfer();
        let t2 = Transfer {
            to: Account {
                owner: bob(),
                subaccount: Some([1; 32]),
            },
            ..simple_transfer()
        };

        assert_ne!(t1.id(), t2.id());
    }

    #[test]
    fn id_unique_over_sender() {
        let t1 = simple_transfer();
        let t2 = Transfer {
            from: Some([1; 32]),
            ..simple_transfer()
        };

        assert_ne!(t1.id(), t2.id());
    }

    #[test]
    fn id_unique_over_created_at() {
        let t1 = simple_transfer();
        let t2 = Transfer {
            created_at: t1.created_at + 1,
            ..simple_transfer()
        };

        assert_ne!(t1.id(), t2.id());
    }

    #[test]
    fn id_unique_over_amount() {
        let t1 = simple_transfer();
        let t2 = Transfer {
            amount: 123.into(),
            ..simple_transfer()
        };

        assert_ne!(t1.id(), t2.id());
    }

    #[test]
    fn id_unique_over_token() {
        let t1 = simple_transfer();
        let t2 = Transfer {
            token: xtc(),
            ..simple_transfer()
        };

        assert_ne!(t1.id(), t2.id());
    }

    #[test]
    fn id_not_unique_over_fee() {
        let t1 = simple_transfer();
        let t2 = Transfer {
            fee: 123.into(),
            ..simple_transfer()
        };

        assert_eq!(t1.id(), t2.id());
    }

    #[test]
    fn effective_fee_considers_type() {
        MockContext::new().with_id(alice()).inject();
        let t = simple_transfer().with_fee(10.into());
        assert_eq!(t.effective_fee(), 10);

        let t = t.double_step();
        assert_eq!(t.effective_fee(), 20);
    }

    #[test]
    fn token_constructor_considers_minter_for_fee() {
        MockContext::new().with_id(alice()).inject();
        let t = Transfer::new(
            &TokenConfiguration {
                principal: bob(),
                fee: 10.into(),
                minting_account: Account {
                    owner: alice(),
                    subaccount: None,
                },
            },
            john(),
            Account {
                owner: john(),
                subaccount: None,
            },
            None,
            1000.into(),
        );

        assert_eq!(t.effective_fee(), 0);

        let t = Transfer::new(
            &TokenConfiguration {
                principal: bob(),
                fee: 10.into(),
                minting_account: Account {
                    owner: john(),
                    subaccount: None,
                },
            },
            john(),
            Account {
                owner: john(),
                subaccount: None,
            },
            None,
            1000.into(),
        );

        assert_eq!(t.effective_fee(), 0);
    }
}
