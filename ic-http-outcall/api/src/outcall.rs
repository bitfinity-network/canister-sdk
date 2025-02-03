//! Abstraction over Http outcalls.

use std::{cell::RefCell, rc::Rc};

use ic_exports::{
    ic_cdk::api::management_canister::http_request::{CanisterHttpRequestArgument, HttpResponse},
    ic_kit::CallResult,
};

/// Abstraction over Http outcalls.
#[allow(async_fn_in_trait)]
pub trait HttpOutcall {
    async fn request(&mut self, args: CanisterHttpRequestArgument) -> CallResult<HttpResponse>;
}

impl<T: HttpOutcall> HttpOutcall for Rc<RefCell<T>> {
    async fn request(&mut self, args: CanisterHttpRequestArgument) -> CallResult<HttpResponse> {
        RefCell::borrow_mut(self).request(args).await
    }
}
