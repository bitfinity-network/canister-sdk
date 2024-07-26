use super::new_test_context;

#[tokio::test]
async fn should_init_tx_cell() {
let ctx = new_test_context().await;
        assert_eq!(ctx.get_tx_from_cell().await.unwrap().from, 0);


}

#[tokio::test]
async fn should_push_tx_to_cell() {
let ctx = new_test_context().await;
        ctx.insert_tx_to_cell(1, 1, 10).await.unwrap();

        assert_eq!(ctx.get_tx_from_cell().await.unwrap().from, 1);


}

#[tokio::test]
async fn should_persist_cell_tx_after_upgrade() {
let ctx = new_test_context().await;
        ctx.insert_tx_to_cell(1, 1, 10).await.unwrap();

        assert_eq!(ctx.get_tx_from_cell().await.unwrap().from, 1);

        super::upgrade_dummy_canister(&ctx).await.unwrap();

        assert_eq!(ctx.get_tx_from_cell().await.unwrap().from, 1);


}
