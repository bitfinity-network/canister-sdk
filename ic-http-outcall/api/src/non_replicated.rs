use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use candid::Principal;
use futures::channel::oneshot;
use ic_canister::virtual_canister_call;
use ic_exports::ic_cdk::api::management_canister::http_request::{
    CanisterHttpRequestArgument, HttpResponse,
};
use ic_exports::ic_kit::{CallResult, RejectionCode};

use crate::outcall::HttpOutcall;
use crate::proxy_types::{RequestArgs, RequestId, REQUEST_METHOD_NAME};
use crate::ResponseResult;

pub type OnResponse = Box<dyn Fn(Vec<ResponseResult>)>;

#[derive(Debug)]
pub struct NonReplicatedHttpOutcall {
    requests: Rc<RefCell<HashMap<RequestId, DeferredResponse>>>,
    callback_api_fn_name: &'static str,
    proxy_canister: Principal,
}

impl NonReplicatedHttpOutcall {
    /// The `callback_api_fn_name` function expected to have the following signature:
    ///
    /// ```
    /// fn(Vec<ResponseResult>) -> ()
    /// ```
    ///
    /// and to call the returned `OnResponse` callback.
    pub fn new(
        proxy_canister: Principal,
        callback_api_fn_name: &'static str,
    ) -> (Self, OnResponse) {
        let s = Self {
            requests: Default::default(),
            callback_api_fn_name,
            proxy_canister,
        };

        let requests = Rc::clone(&s.requests);
        let callback = Box::new(move |responses: Vec<ResponseResult>| {
            for response in responses {
                if let Some(deferred) = requests.borrow_mut().remove(&response.id) {
                    let _ = deferred.notify.send(response.result);
                }
            }
        });

        (s, callback)
    }
}

impl HttpOutcall for NonReplicatedHttpOutcall {
    async fn request(&self, request: CanisterHttpRequestArgument) -> CallResult<HttpResponse> {
        let proxy_canister = self.proxy_canister;
        let request = RequestArgs {
            callback_name: self.callback_api_fn_name.into(),
            request,
        };
        let id: RequestId =
            virtual_canister_call!(proxy_canister, REQUEST_METHOD_NAME, (request,), RequestId)
                .await?;

        let (notify, waker) = oneshot::channel();
        let response = DeferredResponse { notify };
        self.requests.borrow_mut().insert(id, response);

        waker
            .await
            .map_err(|_| {
                // if proxy canister doesn't respond
                (
                    RejectionCode::SysTransient,
                    "timeout waiting HTTP request callback.".into(),
                )
            })?
            .map_err(|e| {
                // if request failed
                (
                    RejectionCode::SysFatal,
                    format!("failed to send HTTP request: {e}"),
                )
            })
    }
}

#[derive(Debug)]
struct DeferredResponse {
    pub notify: oneshot::Sender<Result<HttpResponse, String>>,
}
