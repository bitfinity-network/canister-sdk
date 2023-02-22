use super::*;
use crate::error::ParametersError;

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
    pub from: Account,
    pub to: Account,
    pub amount: Tokens128,
    pub fee: Tokens128,
    pub operation: Operation,
    pub r#type: TransferType,
    pub created_at: Timestamp,
}

#[derive(Debug, CandidType, Deserialize, Clone, Copy)]
pub enum TransferType {
    SingleStep,
    DoubleStep(Stage),
}

#[derive(Debug, CandidType, Deserialize, Clone, Copy)]
pub enum Stage {
    First,
    Second,
}

const INTERMEDIATE_ACC_DOMAIN: &[u8] = b"is-amm-intermediate-acc";

impl Transfer {
    pub(crate) fn id(&self) -> [u8; 32] {
        use ic_exports::ic_crypto_sha::Sha224;

        let mut hash = Sha224::new();
        hash.write(INTERMEDIATE_ACC_DOMAIN);
        hash.write(self.from.owner.as_slice());
        hash.write(self.from.effective_subaccount());
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
        match self.r#type {
            TransferType::SingleStep => self.from.clone(),
            TransferType::DoubleStep(Stage::First) => self.from.clone(),
            TransferType::DoubleStep(Stage::Second) => self.interim_acc(),
        }
    }

    pub(crate) fn to(&self) -> Account {
        match self.r#type {
            TransferType::SingleStep => self.to.clone(),
            TransferType::DoubleStep(Stage::First) => self.interim_acc(),
            TransferType::DoubleStep(Stage::Second) => self.to.clone(),
        }
    }

    fn interim_acc(&self) -> Account {
        Account {
            owner: ic::id().into(),
            subaccount: Some(self.id()),
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.from.owner != ic::id().into() {
            return Err(InternalPaymentError::InvalidParameters(
                ParametersError::NotOwner,
            ));
        }

        if self.from == self.to {
            return Err(InternalPaymentError::InvalidParameters(
                ParametersError::TargetAccountInvalid,
            ));
        }

        if matches!(self.r#type, TransferType::DoubleStep(_))
            && (self.to == self.interim_acc() || self.from == self.interim_acc())
        {
            return Err(InternalPaymentError::InvalidParameters(
                ParametersError::TargetAccountInvalid,
            ));
        }

        let min_amount = self.min_amount(self.fee)?;
        if self.amount < min_amount {
            return Err(InternalPaymentError::InvalidParameters(
                ParametersError::AmountTooSmall {
                    minimum_required: min_amount,
                    actual: self.amount,
                },
            ));
        }

        Ok(())
    }

    fn min_amount(&self, fee: Tokens128) -> Result<Tokens128> {
        let amount = match self.r#type {
            TransferType::DoubleStep(Stage::First) => {
                fee.amount.saturating_mul(2).saturating_add(1)
            }
            _ => fee.amount.saturating_add(1),
        };

        if amount == u128::MAX {
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::FeeTooLarge,
            ))
        } else {
            Ok(amount.into())
        }
    }

    pub fn amount(&self) -> Tokens128 {
        self.amount
    }

    pub fn amount_minus_fee(&self) -> Tokens128 {
        self.amount.saturating_sub(self.fee)
    }

    pub fn operation(&self) -> Operation {
        self.operation
    }

    pub fn caller(&self) -> Principal {
        self.caller
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
            from: Account {
                owner: john().into(),
                subaccount: None,
            },
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
            from: Account {
                owner: john().into(),
                subaccount: None,
            },
            to: Account {
                owner: bob().into(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::DoubleStep(Stage::First),
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
                ParametersError::FeeTooLarge
            ))
        );
    }

    #[test]
    fn validate_second_stage_amount() {
        MockContext::new().with_id(john()).inject();
        let mut transfer = Transfer {
            token: alice(),
            caller: bob(),
            from: Account {
                owner: john().into(),
                subaccount: None,
            },
            to: Account {
                owner: bob().into(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::DoubleStep(Stage::Second),
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
    fn validate_from_not_owner() {
        MockContext::new().with_id(alice()).inject();
        let transfer = Transfer {
            token: alice(),
            caller: bob(),
            from: Account {
                owner: john().into(),
                subaccount: None,
            },
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

        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::NotOwner
            ))
        );
    }

    #[test]
    fn validate_to_self() {
        MockContext::new().with_id(john()).inject();
        let transfer = Transfer {
            token: alice(),
            caller: bob(),
            from: Account {
                owner: john().into(),
                subaccount: Some([1; 32]),
            },
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

    #[test]
    fn validate_to_interim_acc() {
        MockContext::new().with_id(john()).inject();
        let mut transfer = Transfer {
            token: alice(),
            caller: bob(),
            from: Account {
                owner: john().into(),
                subaccount: None,
            },
            to: Account {
                owner: john().into(),
                subaccount: None,
            },
            amount: 1000.into(),
            fee: 0.into(),
            operation: Operation::None,
            r#type: TransferType::SingleStep,
            created_at: 0,
        };

        transfer.to.subaccount = Some(transfer.id());
        assert_eq!(transfer.validate(), Ok(()));

        transfer.r#type = TransferType::DoubleStep(Stage::First);
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::TargetAccountInvalid
            ))
        );

        transfer.r#type = TransferType::DoubleStep(Stage::Second);
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::TargetAccountInvalid
            ))
        );

        transfer.to.subaccount = None;
        transfer.from.subaccount = Some(transfer.id());

        transfer.r#type = TransferType::SingleStep;
        assert_eq!(transfer.validate(), Ok(()));

        transfer.r#type = TransferType::DoubleStep(Stage::First);
        assert_eq!(
            transfer.validate(),
            Err(InternalPaymentError::InvalidParameters(
                ParametersError::TargetAccountInvalid
            ))
        );

        transfer.r#type = TransferType::DoubleStep(Stage::Second);
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
            from: Account {
                owner: john().into(),
                subaccount: None,
            },
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
            from: Account {
                owner: john().into(),
                subaccount: Some([1; 32]),
            },
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
}
