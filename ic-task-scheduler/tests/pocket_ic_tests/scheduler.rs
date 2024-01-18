use std::time::Duration;

use candid::Principal;
use ic_kit::mock_principals::alice;

use crate::pocket_ic_tests::deploy_dummy_scheduler_canister;

thread_local! {
    static CANISTER: std::cell::RefCell<Principal> = std::cell::RefCell::new(Principal::anonymous());
}

#[tokio::test]
async fn test_should_remove_panicking_task() {
    ic_exports::ic_kit::MockContext::new()
        .with_caller(alice())
        .with_id(alice())
        .inject();

    let test_ctx = deploy_dummy_scheduler_canister().await.unwrap();
    CANISTER.with_borrow_mut(|principal| *principal = test_ctx.dummy_scheduler_canister);

    // set error callback
    std::thread::sleep(Duration::from_millis(500));

    // check states
    assert!(test_ctx.save_state_called().await);
    assert!(test_ctx.failed_task_called().await);
}
