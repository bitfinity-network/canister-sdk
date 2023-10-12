use std::any::{Any, TypeId};
use std::collections::BTreeMap;

use candid::utils::{ArgumentDecoder, ArgumentEncoder};
use candid::{self, Principal};
use ic_cdk;

use crate::{CallResponse, Context};

static mut CONTEXT: Option<IcContext> = None;

/// A singleton context that is used in the actual IC environment.
pub struct IcContext {
    /// The storage for this context.
    storage: BTreeMap<TypeId, Box<dyn Any>>,
}

impl IcContext {
    /// Return a mutable reference to the context.
    #[inline(always)]
    pub fn context() -> &'static mut IcContext {
        unsafe {
            if let Some(ctx) = &mut CONTEXT {
                ctx
            } else {
                CONTEXT = Some(IcContext {
                    storage: BTreeMap::new(),
                });
                IcContext::context()
            }
        }
    }

    #[inline(always)]
    fn as_mut(&self) -> &mut Self {
        unsafe {
            let const_ptr = self as *const Self;
            let mut_ptr = const_ptr as *mut Self;
            &mut *mut_ptr
        }
    }
}

impl Context for IcContext {
    #[inline(always)]
    fn trap(&self, message: &str) -> ! {
        ic_cdk::api::trap(message);
    }

    #[inline(always)]
    fn print<S: std::convert::AsRef<str>>(&self, s: S) {
        ic_cdk::api::print(s)
    }

    #[inline(always)]
    fn id(&self) -> Principal {
        ic_cdk::id()
    }

    #[inline(always)]
    fn time(&self) -> u64 {
        ic_cdk::api::time()
    }

    #[inline(always)]
    fn balance(&self) -> u64 {
        ic_cdk::api::canister_balance()
    }

    #[inline(always)]
    fn balance128(&self) -> u128 {
        ic_cdk::api::canister_balance128()
    }

    #[inline(always)]
    fn caller(&self) -> Principal {
        ic_cdk::api::caller()
    }

    #[inline(always)]
    fn msg_cycles_available(&self) -> u64 {
        ic_cdk::api::call::msg_cycles_available()
    }

    #[inline(always)]
    fn msg_cycles_accept(&self, amount: u64) -> u64 {
        ic_cdk::api::call::msg_cycles_accept(amount)
    }

    #[inline(always)]
    fn msg_cycles_refunded(&self) -> u64 {
        ic_cdk::api::call::msg_cycles_refunded()
    }

    #[inline(always)]
    fn store<T: 'static>(&self, data: T) {
        let type_id = TypeId::of::<T>();
        self.as_mut().storage.insert(type_id, Box::new(data));
    }

    #[inline]
    fn get_maybe<T: 'static>(&self) -> Option<&T> {
        let type_id = std::any::TypeId::of::<T>();
        self.storage
            .get(&type_id)
            .map(|b| b.downcast_ref().expect("Unexpected value of invalid type."))
    }

    #[inline(always)]
    fn get_mut<T: 'static + Default>(&self) -> &mut T {
        let type_id = std::any::TypeId::of::<T>();
        self.as_mut()
            .storage
            .entry(type_id)
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("Unexpected value of invalid type.")
    }

    #[inline(always)]
    fn delete<T: 'static + Default>(&self) -> bool {
        let type_id = std::any::TypeId::of::<T>();
        self.as_mut().storage.remove(&type_id).is_some()
    }

    #[inline(always)]
    fn stable_store<T>(&self, data: T) -> Result<(), candid::Error>
    where
        T: ArgumentEncoder,
    {
        ic_cdk::storage::stable_save(data)
    }

    #[inline(always)]
    fn stable_restore<T>(&self) -> Result<T, String>
    where
        T: for<'de> ArgumentDecoder<'de>,
    {
        ic_cdk::storage::stable_restore()
    }

    #[inline(always)]
    fn call_raw<S: Into<String>>(
        &'static self,
        id: Principal,
        method: S,
        args_raw: Vec<u8>,
        cycles: u64,
    ) -> CallResponse<Vec<u8>> {
        let method = method.into();
        Box::pin(async move { ic_cdk::api::call::call_raw(id, &method, &args_raw, cycles).await })
    }

    #[inline(always)]
    fn set_certified_data(&self, data: &[u8]) {
        ic_cdk::api::set_certified_data(data);
    }

    #[inline(always)]
    fn data_certificate(&self) -> Option<Vec<u8>> {
        ic_cdk::api::data_certificate()
    }

    #[inline(always)]
    fn spawn<F: 'static + std::future::Future<Output = ()>>(&mut self, future: F) {
        ic_cdk::spawn(future)
    }
}
