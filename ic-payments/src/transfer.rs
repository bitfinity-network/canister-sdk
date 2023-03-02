use ic_exports::ic_icrc1::Subaccount;

use super::*;
use crate::error::ParametersError;
use crate::icrc1::TokenTransferInfo;

#[derive(Debug, Eq, PartialEq, CandidType, Deserialize, Clone, Copy)]
pub enum Operation {
    None,
    CreditOnSuccess,
    CreditOnError,
}

#[derive(Debug, CandidType, Deserialize, Clone)]
pub struct Transfer {
    pub token: Principal,
    pub caller: Principal,
    pub from: Option<Subaccount>,
    pub to: Account,
    pub amount: Tokens128,
    pub fee: Tokens128,
    pub operation: Operation,
    pub r#type: TransferType,
    pub created_at: Timestamp,
}

#[derive(Debug, CandidType, Deserialize, Clone)]
pub enum TransferType {
    SingleStep,
    DoubleStep(Stage, Account),
}

#[derive(Debug, CandidType, Deserialize, Clone, Copy)]
pub enum Stage {
    First,
    Second,
}

const INTERMEDIATE_ACC_DOMAIN: &[u8] = b"is-amm-intermediate-acc";

impl Transfer {
    pub fn new(
        token_config: &TokenConfiguration,
        caller: Principal,
        to: Account,
        from_subaccount: Option<Subaccount>,
        amount: Tokens128,
    ) -> Self {
        let fee = token_config.get_fee(
            &Account {
                owner: ic::id().into(),
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
        }
    }

    pub fn with_operation(self, operation: Operation) -> Self {
        Self { operation, ..self }
    }

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

    pub async fn execute(&self) -> Result<TokenTransferInfo, InternalPaymentError> {
        icrc1::transfer_icrc1(
            self.token,
            self.to(),
            self.amount_minus_fee(),
            self.fee,
            self.from().subaccount,
            Some(self.created_at()),
        )
        .await
    }

    pub(crate) fn id(&self) -> [u8; 32] {
        use ic_exports::ic_crypto_sha::Sha224;

        let mut hash = Sha224::new();
        hash.write(INTERMEDIATE_ACC_DOMAIN);
        hash.write(&self.from.unwrap_or_default());
        hash.write(self.to.owner.as_slice());
        hash.write(self.to.effective_subaccount());
        hash.write(&self.amount.amount.to_le_bytes());
        hash.write(self.token.as_slice());
        hash.write(&self.created_at.to_le_bytes());

        let hash_result = hash.finish();
        let mut subaccount = [0; 32];
        subaccount[0..4].copy_from_slice(b"vfrc");
        subaccount[4..].copy_from_slice(&hash_result);

        subaccount
    }

    pub(crate) fn from(&self) -> Account {
        match &self.r#type {
            TransferType::SingleStep => self.from_acc(),
            TransferType::DoubleStep(Stage::First, _) => self.from_acc(),
            TransferType::DoubleStep(Stage::Second, acc) => acc.clone(),
        }
    }

    pub fn from_acc(&self) -> Account {
        Account {
            owner: ic::id().into(),
            subaccount: self.from,
        }
    }

    pub fn to(&self) -> Account {
        match &self.r#type {
            TransferType::SingleStep => self.to.clone(),
            TransferType::DoubleStep(Stage::First, acc) => acc.clone(),
            TransferType::DoubleStep(Stage::Second, _) => self.to.clone(),
        }
    }

    pub fn interim_acc(&self) -> Option<Account> {
        match &self.r#type {
            TransferType::DoubleStep(_, acc) => Some(acc.clone()),
            _ => None,
        }
    }

    fn generate_interim_acc(&self) -> Account {
        Account {
            owner: ic::id().into(),
            subaccount: Some(self.id()),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), InternalPaymentError> {
        if self.from_acc() == self.to {
            return Err(InternalPaymentError::InvalidParameters(
                ParametersError::TargetAccountInvalid,
            ));
        }

        if self.final_amount()?.is_zero() {
            return Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: self.min_amount()?,
                    actual: self.amount,
                },
            ));
        }

        Ok(())
    }

    pub fn effective_fee(&self) -> Result<Tokens128, InternalPaymentError> {
        match self.r#type {
            TransferType::DoubleStep(Stage::First, _) => {
                (self.fee * Tokens128::from(2)).to_tokens128().ok_or(
                    InternalPaymentError::InvalidParameters(ParametersError::FeeTooLarge),
                )
            }
            _ => Ok(self.fee),
        }
    }

    fn min_amount(&self) -> Result<Tokens128, InternalPaymentError> {
        (self.effective_fee()? + Tokens128::from(1)).ok_or(InternalPaymentError::InvalidParameters(
            ParametersError::FeeTooLarge,
        ))
    }

    pub fn amount(&self) -> Tokens128 {
        self.amount
    }

    pub fn amount_minus_fee(&self) -> Tokens128 {
        self.amount.saturating_sub(self.fee)
    }

    pub fn final_amount(&self) -> Result<Tokens128, InternalPaymentError> {
        (self.amount - self.effective_fee()?).ok_or(InternalPaymentError::InvalidParameters(
            ParametersError::AmountTooSmall {
                minimum_required: self.min_amount()?,
                actual: self.amount,
            },
        ))
    }

    pub fn operation(&self) -> Operation {
        self.operation
    }

    pub fn caller(&self) -> Principal {
        self.caller
    }

    pub fn renew(self) -> Self {
        Self {
            created_at: ic::time(),
            ..self
        }
    }

    pub fn with_fee(self, fee: Tokens128) -> Self {
        Self { fee, ..self }
    }

    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    pub fn r#type(&self) -> &TransferType {
        &self.r#type
    }

    pub fn next_step(&self) -> Option<Self> {
        match &self.r#type {
            TransferType::DoubleStep(Stage::First, interim_acc) => Some(Self {
                r#type: TransferType::DoubleStep(Stage::Second, interim_acc.clone()),
                amount: self.amount_minus_fee(),
                created_at: ic::time(),
                to: self.to.clone(),
                from: self.from.clone(),
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
                owner: bob().into(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::SingleStep,
            created_at: 0,
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
        transfer.fee = Tokens128::MAX;
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::FeeTooLarge
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
                owner: bob().into(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::DoubleStep(
                Stage::First,
                Account {
                    owner: john().into(),
                    subaccount: Some([1; 32]),
                },
            ),
            created_at: 0,
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
                owner: bob().into(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::DoubleStep(
                Stage::Second,
                Account {
                    owner: john().into(),
                    subaccount: Some([1; 32]),
                },
            ),
            created_at: 0,
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
        transfer.fee = Tokens128::MAX;
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::FeeTooLarge
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
                owner: john().into(),
                subaccount: Some([1; 32]),
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::SingleStep,
            created_at: 0,
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
                owner: bob().into(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::SingleStep,
            created_at: 0,
        }
    }

    #[test]
    fn id_unique_over_reciepient() {
        let t1 = simple_transfer();
        let t2 = Transfer {
            to: Account {
                owner: bob().into(),
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
        assert_eq!(t.effective_fee().unwrap(), 10.into());

        let t = t.double_step();
        assert_eq!(t.effective_fee().unwrap(), 20.into());
    }

    #[test]
    fn token_constructor_considers_minter_for_fee() {
        MockContext::new().with_id(alice()).inject();
        let t = Transfer::new(
            &TokenConfiguration {
                principal: bob(),
                fee: 10.into(),
                minting_account: Account {
                    owner: alice().into(),
                    subaccount: None,
                },
            },
            john(),
            Account {
                owner: john().into(),
                subaccount: None,
            },
            None,
            1000.into(),
        );

        assert_eq!(t.effective_fee().unwrap(), 0.into());

        let t = Transfer::new(
            &TokenConfiguration {
                principal: bob(),
                fee: 10.into(),
                minting_account: Account {
                    owner: john().into(),
                    subaccount: None,
                },
            },
            john(),
            Account {
                owner: john().into(),
                subaccount: None,
            },
            None,
            1000.into(),
        );

        assert_eq!(t.effective_fee().unwrap(), 0.into());
    }
}
