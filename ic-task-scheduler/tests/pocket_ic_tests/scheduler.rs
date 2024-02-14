use candid::Principal;

use crate::pocket_ic_tests::deploy_dummy_scheduler_canister;

thread_local! {
    static CANISTER: std::cell::RefCell<Principal> = std::cell::RefCell::new(Principal::anonymous());
}

#[tokio::test]
async fn test_should_remove_panicking_task() {
    let test_ctx = deploy_dummy_scheduler_canister().await.unwrap();
    CANISTER.with_borrow_mut(|principal| *principal = test_ctx.dummy_scheduler_canister);

    for _ in 0..10 {
        test_ctx.run_scheduler().await;
    }

    assert!(test_ctx.scheduled_state_called().await);
    assert_eq!(test_ctx.executed_tasks().await, vec![3, 0, 1, 2]);
    assert_eq!(test_ctx.panicked_tasks().await, vec![1]);
    assert_eq!(test_ctx.completed_tasks().await, vec![0, 2]);
    assert_eq!(test_ctx.failed_tasks().await, vec![3]);
}
