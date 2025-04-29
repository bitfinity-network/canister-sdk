use std::future::Future;
use std::pin::Pin;

use candid::utils::{ArgumentDecoder, ArgumentEncoder};
use candid::{self, decode_args, encode_args, Principal};
use ic_cdk::call::CallResult;

pub type CallResponse<T> = Pin<Box<dyn Future<Output = CallResult<T>>>>;

pub trait Context {
    /// Trap the code.
    fn trap(&self, message: &str) -> !;

    /// Print a message.
    fn print<S: std::convert::AsRef<str>>(&self, s: S);

    /// ID of the current canister.
    fn id(&self) -> Principal;

    /// The time in nanoseconds.
    fn time(&self) -> u64;

    /// The balance of the canister.
    fn balance(&self) -> u128;

    /// The caller who has invoked this method on the canister.
    fn caller(&self) -> Principal;

    /// Return the number of available cycles that is sent by the caller.
    fn msg_cycles_available(&self) -> u128;

    /// Accept the given amount of cycles, returns the actual amount of accepted cycles.
    fn msg_cycles_accept(&self, amount: u128) -> u128;

    /// Return the cycles that were sent back by the canister that was just called.
    /// This method should only be called right after an inter-canister call.
    fn msg_cycles_refunded(&self) -> u128;

    /// Store the given data to the storage.
    fn store<T: 'static>(&self, data: T);

    /// Return the data that does not implement [`Default`].
    fn get_maybe<T: 'static>(&self) -> Option<&T>;

    /// Return the data associated with the given type. If the data is not present the default
    /// value of the type is returned.
    #[inline]
    fn get<T: 'static + Default>(&self) -> &T {
        self.get_mut()
    }

    /// Return a mutable reference to the given data type, if the data is not present the default
    /// value of the type is constructed and stored. The changes made to the data during updates
    /// is preserved.
    #[allow(clippy::mut_from_ref)]
    fn get_mut<T: 'static + Default>(&self) -> &mut T;

    /// Remove the data associated with the given data type.
    fn delete<T: 'static + Default>(&self) -> bool;

    /// Store the given data to the stable storage.
    fn stable_store<T>(&self, data: T) -> Result<(), candid::Error>
    where
        T: ArgumentEncoder;

    /// Restore the data from the stable storage. If the data is not already stored the None value
    /// is returned.
    fn stable_restore<T>(&self) -> Result<T, String>
    where
        T: for<'de> ArgumentDecoder<'de>;

    /// Perform a call.
    fn call_raw<S: Into<String>>(
        &'static self,
        id: Principal,
        method: S,
        args_raw: Vec<u8>,
        cycles: u128,
    ) -> CallResponse<Vec<u8>>;

    /// Perform the call and return the response.
    #[inline(always)]
    fn call<T: ArgumentEncoder, R: for<'a> ArgumentDecoder<'a>, S: Into<String>>(
        &'static self,
        id: Principal,
        method: S,
        args: T,
    ) -> CallResponse<R> {
        self.call_with_payment(id, method, args, 0)
    }

    #[inline(always)]
    fn call_with_payment<T: ArgumentEncoder, R: for<'a> ArgumentDecoder<'a>, S: Into<String>>(
        &'static self,
        id: Principal,
        method: S,
        args: T,
        cycles: u128,
    ) -> CallResponse<R> {
        let args_raw = encode_args(args).expect("Failed to encode arguments.");
        let method = method.into();
        Box::pin(async move {
            let bytes = self.call_raw(id, method, args_raw, cycles).await?;
            decode_args(&bytes).map_err(|err| panic!("{:?}", err))
        })
    }

    /// Set the certified data of the canister, this method traps if data.len > 32.
    fn set_certified_data(&self, data: &[u8]);

    /// Returns the data certificate authenticating certified_data set by this canister.
    fn data_certificate(&self) -> Option<Vec<u8>>;

    /// Execute a future without blocking the current call.
    fn spawn<F: 'static + std::future::Future<Output = ()>>(&mut self, future: F);
}
