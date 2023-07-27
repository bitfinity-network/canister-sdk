use crate::candid::utils::{ArgumentDecoder, ArgumentEncoder};
use crate::ic;
use crate::{CallResponse, Principal};

pub mod management;

/// A method description.
pub trait Method {
    const NAME: &'static str;
    type Arguments: ArgumentEncoder;
    type Response: for<'de> ArgumentDecoder<'de>;

    #[inline]
    fn perform(id: Principal, args: Self::Arguments) -> CallResponse<Self::Response> {
        ic::call(id, Self::NAME, args)
    }

    #[inline]
    fn perform_with_payment(
        id: Principal,
        args: Self::Arguments,
        cycles: u64,
    ) -> CallResponse<Self::Response> {
        ic::call_with_payment(id, Self::NAME, args, cycles)
    }
}
