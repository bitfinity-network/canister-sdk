use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use candid::Principal;
use ic_canister::{init, query, update, virtual_canister_call, Canister, PreUpdate};
use ic_exports::ic_cdk::api::management_canister::http_request::CanisterHttpRequestArgument;
use ic_exports::ic_kit::ic;
use ic_http_outcall_api::{InitArgs, RequestArgs, RequestId, ResponseResult};

static IDS_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Canister)]
#[canister_no_upgrade_methods]
pub struct HttpProxyCanister {
    #[id]
    principal: Principal,
}

impl PreUpdate for HttpProxyCanister {}

impl HttpProxyCanister {
    #[init]
    pub async fn init(&mut self, args: InitArgs) {
        ALLOWED_PROXY.with(move |cell| {
            cell.get_or_init(move || args.allowed_proxy);
        });
    }

    #[update]
    pub fn http_outcall(&mut self, args: RequestArgs) -> RequestId {
        let sender = ic::caller();
        let id = IDS_COUNTER.fetch_add(1, Ordering::Relaxed).into();
        let with_sender = RequestWithSender { args, sender };

        PENDING_REQUESTS.with_borrow_mut(move |map| map.insert(id, with_sender));

        id
    }

    #[query]
    pub fn pending_requests(&self, limit: usize) -> Vec<(RequestId, CanisterHttpRequestArgument)> {
        check_allowed_proxy(ic::caller());

        PENDING_REQUESTS.with_borrow(|map| {
            map.iter()
                .take(limit)
                .map(|(k, v)| (*k, v.args.request.clone()))
                .collect()
        })
    }

    #[update]
    pub async fn finish_requests(&mut self, responses: Vec<ResponseResult>) {
        check_allowed_proxy(ic::caller());

        for response in responses {
            let Some(request) = PENDING_REQUESTS.with_borrow_mut(|map| map.remove(&response.id))
            else {
                continue;
            };

            ic::spawn(async move {
                let _ = virtual_canister_call!(
                    request.sender,
                    &request.args.callback_name,
                    (response,),
                    ()
                )
                .await;
            });
        }
    }
}

fn check_allowed_proxy(proxy: Principal) {
    let allowed_proxy = ALLOWED_PROXY
        .with(|val| val.get().copied())
        .expect("allowed proxy to be initialized");

    if proxy != allowed_proxy {
        ic::trap("only allowed proxy may process requests")
    }
}

#[derive(Debug, Clone)]
struct RequestWithSender {
    pub args: RequestArgs,
    pub sender: Principal,
}

thread_local! {
    static ALLOWED_PROXY: OnceCell<Principal> = const { OnceCell::new() };
    static PENDING_REQUESTS: RefCell<HashMap<RequestId, RequestWithSender>> = RefCell::default();
}

#[cfg(test)]
mod tests {

    use candid::{Nat, Principal};
    use ic_canister::{canister_call, register_virtual_responder, Canister};
    use ic_exports::{
        ic_cdk::api::management_canister::http_request::{
            CanisterHttpRequestArgument, HttpMethod, HttpResponse,
        },
        ic_kit::MockContext,
    };
    use ic_http_outcall_api::{InitArgs, RequestArgs, ResponseResult};
    use tokio::sync::mpsc::channel;

    use crate::HttpProxyCanister;

    #[tokio::test]
    async fn http_proxy_canister_works() {
        let ctx = MockContext::new().inject();

        let ctx_canister = Principal::from_slice(&[1, 2, 3, 4, 5]);
        ctx.update_id(ctx_canister);

        let (finished_ids_tx, mut finished_ids_rx) = channel(4);
        let callback_name = "some_callback";
        register_virtual_responder(
            ctx_canister,
            callback_name,
            move |(response,): (ResponseResult,)| {
                assert!(response.result.is_ok());

                let tx = finished_ids_tx.clone();
                tokio::spawn(async move { tx.send(response.id).await.unwrap() });
            },
        );

        let mut proxy_canister = HttpProxyCanister::init_instance();
        proxy_canister
            .init(InitArgs {
                allowed_proxy: ctx_canister,
            })
            .await;

        let request = CanisterHttpRequestArgument {
            url: "https://example.com/".into(),
            method: HttpMethod::GET,
            ..Default::default()
        };
        let request_args = RequestArgs {
            callback_name: callback_name.into(),
            request,
        };
        let id = canister_call!(proxy_canister.http_outcall(request_args), RequestId)
            .await
            .unwrap();

        let mut pending_requests = canister_call!(
            proxy_canister.pending_requests(10),
            Vec<(RequestId, CanisterHttpRequestArgument)>
        )
        .await
        .unwrap();

        let request = pending_requests.remove(0);
        assert_eq!(request.0, id);

        let response = ResponseResult {
            id,
            result: Ok(HttpResponse {
                status: Nat::from(200u64),
                headers: vec![],
                body: vec![],
            }),
        };
        canister_call!(proxy_canister.finish_requests(vec![response]), ())
            .await
            .unwrap();

        assert_eq!(finished_ids_rx.recv().await.unwrap(), id);
    }
}
