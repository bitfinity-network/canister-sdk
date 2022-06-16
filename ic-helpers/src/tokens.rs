use auto_ops::impl_op_ex;
use candid::types::{Serializer, Type};
use candid::{CandidType, Deserialize};
use crypto_bigint::{CheckedAdd, CheckedMul, CheckedSub, Limb, NonZero, U256};
use num_traits::SaturatingSub;
use serde::de::{Error, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserializer, Serialize};
use std::fmt::{Display, Formatter};

#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    CandidType,
    Deserialize,
    Serialize,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
)]
pub struct Tokens128 {
    pub amount: u128,
}

impl Tokens128 {
    pub const ZERO: Tokens128 = Tokens128 { amount: 0 };
    pub const MAX: Tokens128 = Tokens128 { amount: u128::MAX };

    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    pub fn from_f64(amount: f64) -> Option<Self> {
        if amount < 0.0 || amount > u128::MAX as f64 {
            None
        } else {
            Some(Self {
                amount: amount as u128,
            })
        }
    }

    pub fn to_f64(&self) -> f64 {
        self.amount as f64
    }

    pub fn to_u64(&self) -> Option<u64> {
        if self.amount > u64::MAX as u128 {
            None
        } else {
            Some(self.amount as u64)
        }
    }

    // we don't use the trait here because the `Sub` trait implementation returns an option
    pub fn saturating_sub(&self, v: Self) -> Self {
        Self {
            amount: self.amount.saturating_sub(v.amount),
        }
    }
}

impl_op_ex!(+ |a: &Tokens128, b: &Tokens128| -> Option<Tokens128> { Some(Tokens128::from(a.amount.checked_add(b.amount)?)) });
impl_op_ex!(-|a: &Tokens128, b: &Tokens128| -> Option<Tokens128> {
    Some(Tokens128::from(a.amount.checked_sub(b.amount)?))
});
impl_op_ex!(*|a: &Tokens128, b: &Tokens128| -> Tokens256 {
    Tokens256(U256::from(a.amount).saturating_mul(&U256::from(b.amount)))
});
impl_op_ex!(*|a: &Tokens128, b: &u128| -> Tokens256 { a * Tokens128::from(*b) });
impl_op_ex!(*|a: &Tokens128, b: &u64| -> Tokens256 { a * Tokens128::from(*b as u128) });
impl_op_ex!(*|a: &Tokens128, b: &u32| -> Tokens256 { a * Tokens128::from(*b as u128) });
impl_op_ex!(*|a: &Tokens128, b: &usize| -> Tokens256 { a * Tokens128::from(*b as u128) });

impl From<u128> for Tokens128 {
    fn from(amount: u128) -> Self {
        Self { amount }
    }
}

impl From<Tokens128> for f64 {
    fn from(amount: Tokens128) -> Self {
        amount.amount as f64
    }
}

impl Display for Tokens128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.amount.fmt(f)
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Tokens256(pub U256);

impl_op_ex!(+ |a: &Tokens256, b: &Tokens256| -> Option<Tokens256> {
    let inner = CheckedAdd::checked_add(&a.0, &b.0);
    if inner.is_some().into() {
        Some(Tokens256(inner.unwrap()))
    } else {
        None
    }
});
impl_op_ex!(+ |a: &Tokens256, b: &Tokens128| -> Option<Tokens256> {a + Tokens256::from(*b)});

impl_op_ex!(-|a: &Tokens256, b: &Tokens256| -> Option<Tokens256> {
    let inner = CheckedSub::checked_sub(&a.0, &b.0);
    if inner.is_some().into() {
        Some(Tokens256(inner.unwrap()))
    } else {
        None
    }
});

impl_op_ex!(*|a: &Tokens256, b: &Tokens256| -> Option<Tokens256> {
    let inner = CheckedMul::checked_mul(&a.0, &b.0);
    if inner.is_some().into() {
        Some(Tokens256(inner.unwrap()))
    } else {
        None
    }
});
impl_op_ex!(*|a: &Tokens256, b: &u128| -> Option<Tokens256> { a * Tokens256::from(*b) });
impl_op_ex!(*|a: &Tokens256, b: &u64| -> Option<Tokens256> { a * Tokens256::from(*b as u128) });
impl_op_ex!(*|a: &Tokens256, b: &u32| -> Option<Tokens256> { a * Tokens256::from(*b as u128) });
impl_op_ex!(*|a: &Tokens256, b: &usize| -> Option<Tokens256> { a * Tokens256::from(*b as u128) });

impl_op_ex!(/ |a: &Tokens256, b: &Tokens256| -> Option<Tokens256> {
    let inner = a.0.checked_div(&b.0);
    if inner.is_some().into() {
        Some(Tokens256(inner.unwrap()))
    } else {
        None
    }
});

impl_op_ex!(/ |a: &Tokens256, b: &Tokens128| -> Option<Tokens256> {
    a / b.amount
});

impl_op_ex!(/ |a: &Tokens256, b: &u128| -> Option<Tokens256> {
    let b_checked = NonZero::from_u128(std::num::NonZeroU128::new(*b)?);
    Some(Tokens256(a.0 / b_checked))
});

impl_op_ex!(/ |a: &Tokens256, b: &u64| -> Option<Tokens256> { a / *b as u128 });

impl Tokens256 {
    pub const ZERO: Tokens256 = Tokens256(U256::ZERO);
    pub const MAX: Tokens256 = Tokens256(U256::MAX);

    pub fn to_tokens128(&self) -> Option<Tokens128> {
        let limbs = self.0.limbs();
        if limbs[2].0 != 0 || limbs[3].0 != 0 {
            return None;
        }

        let num = limbs[0].0 as u128 + limbs[1].0 as u128 * (u64::MAX as u128 + 1);
        Some(Tokens128::from(num))
    }

    pub fn sqrt(&self) -> Self {
        Self(self.0.sqrt())
    }

    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    pub fn to_f64(&self) -> f64 {
        let mut val = 0.0;
        for (i, limb) in self.0.limbs().iter().enumerate() {
            val += limb.0 as f64 * (u64::MAX as f64).powi(i as i32);
        }

        val
    }
}

impl From<u128> for Tokens256 {
    fn from(amount: u128) -> Self {
        Self(amount.into())
    }
}

impl From<Tokens128> for Tokens256 {
    fn from(amount: Tokens128) -> Self {
        amount.amount.into()
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

impl Serialize for Tokens256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(4))?;
        for limb in self.0.limbs() {
            seq.serialize_element(&u64::from(limb.0))?;
        }

        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candid::{Decode, Encode};
    use crypto_bigint::CheckedMul;

    #[test]
    fn tokens_u128_add() {
        assert_eq!(
            Tokens128::from(0) + Tokens128::from(0),
            Some(Tokens128::from(0))
        );
        assert_eq!(
            Tokens128::from(12345) + Tokens128::from(6789),
            Some(Tokens128::from(12345 + 6789))
        );
        assert_eq!(
            Tokens128::from(u128::MAX) + Tokens128::from(0),
            Some(Tokens128::from(u128::MAX))
        );
        assert_eq!(Tokens128::from(u128::MAX) + Tokens128::from(1), None);
        assert_eq!(
            Tokens128::from(u128::MAX) + Tokens128::from(u128::MAX),
            None
        );
    }

    #[test]
    fn tokens_u128_sum() {
        assert_eq!(
            Tokens128::from(0) - Tokens128::from(0),
            Some(Tokens128::from(0))
        );
        assert_eq!(
            Tokens128::from(12345) - Tokens128::from(6789),
            Some(Tokens128::from(12345 - 6789))
        );
        assert_eq!(
            Tokens128::from(u128::MAX) - Tokens128::from(u128::MAX),
            Some(Tokens128::from(0))
        );
        assert_eq!(Tokens128::from(0) - Tokens128::from(1), None);
        assert_eq!(
            Tokens128::from(u128::MAX - 1) - Tokens128::from(u128::MAX),
            None
        );
    }

    #[test]
    fn tokens_u128_mul() {
        assert_eq!(
            Tokens128::from(0) * Tokens128::from(0),
            Tokens256(U256::ZERO)
        );
        assert_eq!(
            Tokens128::from(1) * Tokens128::from(1),
            Tokens256(U256::ONE)
        );
        assert_eq!(
            Tokens128::from(u128::MAX) * Tokens128::from(u128::MAX),
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
        assert_eq!(
            Tokens256(U256::ZERO).to_tokens128(),
            Some(Tokens128::from(0))
        );
        assert_eq!(
            Tokens256(U256::ONE).to_tokens128(),
            Some(Tokens128::from(1))
        );
        assert_eq!(
            Tokens256(U256::from(u128::MAX)).to_tokens128(),
            Some(Tokens128::from(u128::MAX))
        );
        assert_eq!(
            Tokens256(U256::from(u128::MAX).saturating_add(&U256::from(1u128))).to_tokens128(),
            None
        );
    }

    #[test]
    fn token256_to_f64() {
        let num = (Tokens256::from(u128::MAX) * 100500u128).unwrap();
        let expected = u128::MAX as f64 * 100500.0;
        let converted = num.to_f64();
        assert_eq!(converted, expected);
    }
}
