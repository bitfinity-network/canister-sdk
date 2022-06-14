use auto_ops::impl_op_ex;
use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use crypto_bigint::{Limb, NonZero, U256};
use serde::de::{Error, SeqAccess, Visitor};
use serde::Deserializer;
use std::fmt::Formatter;

#[derive(
    Default, Debug, Clone, Copy, CandidType, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash,
)]
pub struct Tokens128 {
    pub amount: u128,
}

impl_op_ex!(+ |a: &Tokens128, b: &Tokens128| -> Option<Tokens128> { Some(Tokens128::from(a.amount.checked_add(b.amount)?)) });
impl_op_ex!(-|a: &Tokens128, b: &Tokens128| -> Option<Tokens128> {
    Some(Tokens128::from(a.amount.checked_sub(b.amount)?))
});
impl_op_ex!(*|a: &Tokens128, b: &Tokens128| -> Tokens256 {
    Tokens256(U256::from(a.amount).saturating_mul(&U256::from(b.amount)))
});
impl_op_ex!(*|a: &Tokens128, b: &u64| -> Tokens256 { a * Tokens128::from(*b as u128) });
impl_op_ex!(*|a: &Tokens128, b: &u32| -> Tokens256 { a * Tokens128::from(*b as u128) });
impl_op_ex!(*|a: &Tokens128, b: &usize| -> Tokens256 { a * Tokens128::from(*b as u128) });

impl From<u128> for Tokens128 {
    fn from(amount: u128) -> Self {
        Self { amount }
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Tokens256(U256);

impl_op_ex!(/ |a: &Tokens256, b: &Tokens128| -> Option<Tokens256> {
    a / b.amount
});

impl_op_ex!(/ |a: &Tokens256, b: &u128| -> Option<Tokens256> {
    let b_checked = NonZero::from_u128(std::num::NonZeroU128::new(*b)?);
    Some(Tokens256(a.0 / b_checked))
});

impl_op_ex!(/ |a: &Tokens256, b: &u64| -> Option<Tokens256> { a / *b as u128 });

impl Tokens256 {
    pub fn to_tokens128(&self) -> Option<Tokens128> {
        let limbs = self.0.limbs();
        if limbs[2].0 != 0 || limbs[3].0 != 0 {
            return None;
        }

        let num = limbs[0].0 as u128 + limbs[1].0 as u128 * (u64::MAX as u128 + 1);
        Some(Tokens128::from(num))
    }
}

impl CandidType for Tokens256 {
    fn _ty() -> Type {
        Type::Vec(Box::new(Type::Nat64))
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer + Serializer,
    {
        self.0
            .limbs()
            .iter()
            .map(|limb| limb.0)
            .collect::<Vec<u64>>()
            .idl_serialize(serializer)
    }
}

struct Tokens256Visitor;

impl<'de> Visitor<'de> for Tokens256Visitor {
    type Value = Vec<u64>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("vector of u64 of the length 4")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut numbers: Vec<u64> = vec![];
        for i in 0..4 {
            numbers.push(
                seq.next_element()?
                    .ok_or_else(|| A::Error::invalid_length(i, &self))?,
            );
        }

        if seq.next_element::<u64>()?.is_some() {
            return Err(A::Error::invalid_length(5, &self));
        }

        Ok(numbers)
    }
}

impl<'de> Deserialize<'de> for Tokens256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let numbers = deserializer
            .deserialize_seq(Tokens256Visitor)
            .expect("couldn't read numbers vec");
        let limbs = numbers.iter().map(|n| Limb(*n)).collect::<Vec<Limb>>();
        let mut limbs_fixed: [Limb; 4] = [Limb::ZERO; 4];
        limbs_fixed.copy_from_slice(&limbs);
        Ok(Tokens256(U256::new(limbs_fixed)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candid::{Decode, Encode};
    use crypto_bigint::CheckedMul;

    #[test]
    fn tokens_u128_add() {
        assert_eq!(Tokens128(0) + Tokens128(0), Some(Tokens128(0)));
        assert_eq!(
            Tokens128(12345) + Tokens128(6789),
            Some(Tokens128(12345 + 6789))
        );
        assert_eq!(
            Tokens128(u128::MAX) + Tokens128(0),
            Some(Tokens128(u128::MAX))
        );
        assert_eq!(Tokens128(u128::MAX) + Tokens128(1), None);
        assert_eq!(Tokens128(u128::MAX) + Tokens128(u128::MAX), None);
    }

    #[test]
    fn tokens_u128_sum() {
        assert_eq!(Tokens128(0) - Tokens128(0), Some(Tokens128(0)));
        assert_eq!(
            Tokens128(12345) - Tokens128(6789),
            Some(Tokens128(12345 - 6789))
        );
        assert_eq!(
            Tokens128(u128::MAX) - Tokens128(u128::MAX),
            Some(Tokens128(0))
        );
        assert_eq!(Tokens128(0) - Tokens128(1), None);
        assert_eq!(Tokens128(u128::MAX - 1) - Tokens128(u128::MAX), None);
    }

    #[test]
    fn tokens_u128_mul() {
        assert_eq!(Tokens128(0) * Tokens128(0), Tokens256(U256::ZERO));
        assert_eq!(Tokens128(1) * Tokens128(1), Tokens256(U256::ONE));
        assert_eq!(
            Tokens128(u128::MAX) * Tokens128(u128::MAX),
            Tokens256(
                U256::from(u128::MAX)
                    .checked_mul(&U256::from(u128::MAX))
                    .unwrap()
            )
        );
    }

    #[test]
    fn u256_serialization() {
        let num = Tokens256(U256::MAX);
        let serialized = Encode!(&num).unwrap();
        let deserialized = Decode!(&serialized, Tokens256).unwrap();
        assert_eq!(deserialized, num);
    }

    #[test]
    fn tokens256_to_tokens128() {
        assert_eq!(Tokens256(U256::ZERO).to_tokens128(), Some(Tokens128(0)));
        assert_eq!(Tokens256(U256::ONE).to_tokens128(), Some(Tokens128(1)));
        assert_eq!(
            Tokens256(U256::from(u128::MAX)).to_tokens128(),
            Some(Tokens128(u128::MAX))
        );
        assert_eq!(
            Tokens256(U256::from(u128::MAX).saturating_add(&U256::from(1u128))).to_tokens128(),
            None
        );
    }
}
