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

    for _ in 0..10 {
        test_ctx.run_scheduler().await;
    }

    assert!(test_ctx.scheduled_state_called().await);
    assert_eq!(test_ctx.executed_tasks().await, vec![0, 2, 1, 3, 3, 2, 1]);
    assert_eq!(test_ctx.panicked_tasks().await, vec![1]);
    assert_eq!(test_ctx.completed_tasks().await, vec![0, 2]);
    assert_eq!(test_ctx.failed_tasks().await, vec![3]);
}
