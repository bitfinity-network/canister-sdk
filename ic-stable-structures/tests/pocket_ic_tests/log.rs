use super::new_test_context;

#[tokio::test]
async fn should_init_tx_log() {
let ctx = new_test_context().await;
        let res = ctx.get_tx_from_log(0).await.unwrap();
        assert!(res.is_some());

}

#[tokio::test]
async fn should_push_tx_to_log() {
let ctx = new_test_context().await;
        ctx.push_tx_to_log(1, 1, 10).await.unwrap();

        assert!(ctx.get_tx_from_log(1).await.unwrap().is_some());


}

#[tokio::test]
async fn should_persist_log_tx_after_upgrade() {
let ctx = new_test_context().await;
        ctx.push_tx_to_log(1, 1, 10).await.unwrap();

        assert!(ctx.get_tx_from_log(1).await.unwrap().is_some());

        super::upgrade_dummy_canister(&ctx).await.unwrap();

        assert!(ctx.get_tx_from_log(0).await.unwrap().is_some());
        assert!(ctx.get_tx_from_log(1).await.unwrap().is_some());


}
