use std::collections::HashMap;

use candid::Principal;
use futures::channel::oneshot;
use ic_canister::virtual_canister_call;
use ic_exports::{
    ic_cdk::api::management_canister::http_request::{CanisterHttpRequestArgument, HttpResponse},
    ic_kit::CallResult,
};

use crate::{
    outcall::HttpOutcall,
    proxy_types::{OnResponseArgs, RequestArgs, RequestId, REQUEST_METHOD_NAME},
};

#[derive(Debug)]
pub struct NonReplicatedHttpOutcall {
    requests: HashMap<RequestId, DeferredResponse>,
    callback_api_fn_name: &'static str,
    proxy_canister: Principal,
}

impl NonReplicatedHttpOutcall {
    pub fn new(proxy_canister: Principal, callback_api_fn_name: &'static str) -> Self {
        Self {
            requests: Default::default(),
            callback_api_fn_name,
            proxy_canister,
        }
    }

    pub fn on_response(&mut self, args: OnResponseArgs) {
        if let Some(response) = self.requests.remove(&args.request_id) {
            let _ = response.notify.send(args.response);
        }
    }
}

impl HttpOutcall for NonReplicatedHttpOutcall {
    async fn request(&mut self, request: CanisterHttpRequestArgument) -> CallResult<HttpResponse> {
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
        self.requests.insert(id, response);

        Ok(waker.await.unwrap_or_else(|_| HttpResponse {
            status: 408_u64.into(), // timeout error
            headers: vec![],
            body: vec![],
        }))
    }
}

#[derive(Debug)]
struct DeferredResponse {
    pub notify: oneshot::Sender<HttpResponse>,
}
