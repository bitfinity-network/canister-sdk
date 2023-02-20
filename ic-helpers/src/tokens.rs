use std::fmt::{Display, Formatter};
use std::mem::size_of;

use auto_ops::impl_op_ex;
use candid::Nat;
use crypto_bigint::{CheckedAdd, CheckedMul, CheckedSub, NonZero, U256};
use ic_exports::ic_cdk::export::candid::types::{Serializer, Type, TypeId};
use ic_exports::ic_cdk::export::candid::{self, CandidType, Deserialize};
use num_bigint::BigUint;
use num_traits::{FromPrimitive, ToPrimitive};
use serde::de::{Error, Unexpected};
use serde::{Deserializer, Serialize};

/// Token amount limited by the value of u128::MAX (2^128 - 1).
///
/// This structure does not specify the number of decimal places after the point, and thus can be
/// used to represent 8 decimal places (like in BTC or ICP) or 18 decimal places (like in ETH).
///
/// All the arithmetic operation are specifically designed to check for any overflows/underflows and
/// make all the bound checks explicit for the consumer.
///
/// **Note**: this struct exists explicitly to remove the burden of constantly calling
/// `u128::checked_add` etc.
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Tokens128 {
    pub amount: u128,
}

impl CandidType for Tokens128 {
    fn _ty() -> Type {
        Type::Nat
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_nat(&candid::types::number::Nat::from(self.amount))
    }
}

impl Serialize for Tokens128 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u128(self.amount)
    }
}

impl<'de> Deserialize<'de> for Tokens128 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nat = candid::Nat::deserialize(deserializer)?;
        Self::from_nat(&nat).ok_or_else(|| {
            D::Error::invalid_value(Unexpected::Str(&nat.to_string()), &"value is too large")
        })
    }
}

impl Tokens128 {
    /// Zero value.
    pub const ZERO: Tokens128 = Tokens128 { amount: 0 };

    /// Max value.
    pub const MAX: Tokens128 = Tokens128 { amount: u128::MAX };

    /// Returns true if the amount of the tokens is 0.
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    /// Converts f64 value to the tokens value. Returns None if the value is negative or larger than 128::MAX.
    pub fn from_f64(amount: f64) -> Option<Self> {
        if amount < 0.0 || amount > u128::MAX as f64 {
            None
        } else {
            Some(Self {
                amount: amount as u128,
            })
        }
    }

    /// Lossy conversion to f64. If the value cannot be expressed exactly by f64, the mantissa will
    /// be floored.
    pub fn to_f64(&self) -> f64 {
        self.amount as f64
    }

    /// Converts the value to u64. Returns None if the value is greater than u64::MAX.
    pub fn to_u64(&self) -> Option<u64> {
        if self.amount > u64::MAX as u128 {
            None
        } else {
            Some(self.amount as u64)
        }
    }

    /// Subtracts `other` from `self`, returning Tokens128::ZERO on underflow.
    pub fn saturating_sub(&self, other: Self) -> Self {
        // we don't use the trait here because the `Sub` trait implementation returns an option
        Self {
            amount: self.amount.saturating_sub(other.amount),
        }
    }

    /// Adds `other` to `self` returning Tokens128::MAX on overflow.
    pub fn saturating_add(&self, other: Self) -> Self {
        match self + other {
            Some(v) => v,
            None => Self::MAX,
        }
    }

    pub fn from_nat(nat: &candid::Nat) -> Option<Self> {
        let mut bytes = nat.0.to_bytes_le();
        if bytes.len() > size_of::<u128>() {
            None
        } else {
            bytes.resize(size_of::<u128>(), 0);
            Some(Self {
                amount: u128::from_le_bytes(bytes.try_into().ok()?),
            })
        }
    }
}

impl_op_ex!(+ |a: &Tokens128, b: &Tokens128| -> Option<Tokens128> { Some(Tokens128::from(a.amount.checked_add(b.amount)?)) });
impl_op_ex!(-|a: &Tokens128, b: &Tokens128| -> Option<Tokens128> {
    Some(Tokens128::from(a.amount.checked_sub(b.amount)?))
});
impl_op_ex!(-|a: &Tokens128, b: &u128| -> Option<Tokens128> { a - Tokens128::from(*b) });
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

impl From<Tokens128> for Nat {
    fn from(amount: Tokens128) -> Self {
        amount.into()
    }
}

impl Display for Tokens128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.amount.fmt(f)
    }
}

/// Token amount limited by the value of u256 (2^256 - 1).
///
/// This structure has fixed memory size and thus is Copy.
///
/// This structure does not specify the number of decimal places after the point, and thus can be
/// used to represent 8 decimal places (like in BTC or ICP) or 18 decimal places (like in ETH).
///
/// All the arithmetic operation are specifically designed to check for any overflows/underflows and
/// make all the bound checks explicit for the consumer.
///
/// The intended use of this struct is to return aggregated values of Tokens128. As such, it is
/// usually not used as an input value in APIs, os it's serialized by Candid to `Nat`.
#[derive(Default, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
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
    /// Zero value.
    pub const ZERO: Tokens256 = Tokens256(U256::ZERO);

    /// Max possible value.
    pub const MAX: Tokens256 = Tokens256(U256::MAX);

    /// Number of bytes needed to represent the value.
    pub const BYTE_LENGTH: usize = 256 / 8;

    /// Converts the value to Tokens128. Returns `None` if the value is greater than `Tokens128::MAX`.
    pub fn to_tokens128(&self) -> Option<Tokens128> {
        let limbs = self.0.limbs();
        if limbs[2].0 != 0 || limbs[3].0 != 0 {
            return None;
        }

        let num = limbs[0].0 as u128 + limbs[1].0 as u128 * (u64::MAX as u128 + 1);
        Some(Tokens128::from(num))
    }

    /// Rounded square root of the value.
    pub fn sqrt(&self) -> Self {
        Self(self.0.sqrt())
    }

    /// Returns true if the value equals zero.
    pub fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    /// Lossy conversion to f64. If the value cannot be expressed exactly by f64, the mantissa will
    /// be floored.
    pub fn to_f64(&self) -> f64 {
        let mut val = 0.0;
        for (i, limb) in self.0.limbs().iter().enumerate() {
            val += limb.0 as f64 * (u64::MAX as f64).powi(i as i32);
        }

        val
    }

    /// Converts f64 value to the tokens value. Returns None if the value is negative or larger than `Tokens256::MAX`.
    pub fn from_f64(amount: f64) -> Option<Self> {
        let bigint = BigUint::from_f64(amount)?;
        Self::from_nat(&candid::Nat(bigint))
    }

    /// Converts the value to `candid::Nat`.
    pub fn to_nat(&self) -> candid::Nat {
        let limbs = self
            .0
            .limbs()
            .map(|l| l.0.to_usize().expect("never panics"));
        let mut nums = vec![];
        for limb in limbs.into_iter() {
            // We use little endian since WASM is little endian
            nums.append(&mut limb.to_le_bytes().to_vec());
        }

        candid::Nat(BigUint::from_bytes_le(&nums))
    }

    /// Constructs the value from `candid::Nat`. Returns `None` if the `Nat` value is greater than
    /// `Tokens256::MAX`.
    pub fn from_nat(nat: &candid::Nat) -> Option<Self> {
        let mut bytes = nat.0.to_bytes_le();
        if bytes.len() > Self::BYTE_LENGTH {
            None
        } else {
            bytes.resize(Self::BYTE_LENGTH, 0);
            Some(Self(U256::from_le_slice(&bytes)))
        }
    }

    /// Adds `other` to `self` returning Tokens128::MAX on overflow.
    pub fn saturating_add(&self, other: Self) -> Self {
        match *self + other {
            Some(v) => v,
            None => Self::MAX,
        }
    }

    /// Subtracts `other` from `self`, returning Tokens128::ZERO on underflow.
    pub fn saturating_sub(&self, other: Self) -> Self {
        match *self - other {
            Some(v) => v,
            None => Self::ZERO,
        }
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
    fn id() -> TypeId {
        TypeId::of::<candid::Nat>()
    }
    fn _ty() -> Type {
        Type::Nat
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer + Serializer,
    {
        serializer.serialize_nat(&self.to_nat())
    }
}

impl<'de> Deserialize<'de> for Tokens256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nat = candid::Nat::deserialize(deserializer)?;
        Self::from_nat(&nat).ok_or_else(|| {
            D::Error::invalid_value(Unexpected::Str(&nat.to_string()), &"value is too large")
        })
    }
}

// Default U256 debug prints the number as a set of limbs, and to_string() uses hex formatting, which is inconvinient
// for debugging. This implementation writes the inner number as decimal. It's not that fast but it shouldn't be
// a problem since it's supposed to be used only for debugging.
impl std::fmt::Debug for Tokens256 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let nat = self.to_nat().0;
        f.debug_tuple("Tokens256")
            .field(&format_args!("{nat}"))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use candid::{Decode, Encode};
    use crypto_bigint::CheckedMul;

    use super::*;

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

    #[test]
    fn tokens256_to_nat() {
        let num = U256::from(u128::MAX)
            .saturating_mul(&U256::from(47u128))
            .saturating_mul(&U256::from(u64::MAX));
        let converted = Tokens256(num).to_nat();
        let expected =
            candid::Nat(BigUint::from(u128::MAX) * BigUint::from(47u128) * BigUint::from(u64::MAX));
        assert_eq!(converted, expected);
    }

    #[test]
    fn tokens256_serialization() {
        let num = U256::from(u128::MAX)
            .saturating_mul(&U256::from(47u128))
            .saturating_mul(&U256::from(u64::MAX));
        let tokens = Tokens256(num);
        let serialized = candid::Encode!(&tokens).unwrap();
        let deserialized = candid::Decode!(&serialized, Tokens256).unwrap();
        assert_eq!(deserialized, tokens);
    }

    #[test]
    fn tokens256_debug() {
        let num = Tokens256::from(123);
        assert_eq!(&format!("{num:?}"), "Tokens256(123)");
        let num = Tokens256::from(100500);
        assert_eq!(&format!("{num:?}"), "Tokens256(100500)");
        let num = Tokens256::from(0);
        assert_eq!(&format!("{num:?}"), "Tokens256(0)");
        let num = (Tokens256::from(u128::MAX) + Tokens256::from(1)).unwrap();
        assert_eq!(
            &format!("{num:?}"),
            "Tokens256(340282366920938463463374607431768211456)"
        );
        let num = Tokens256::MAX;
        assert_eq!(&format!("{num:?}"), "Tokens256(115792089237316195423570985008687907853269984665640564039457584007913129639935)");
    }
}
