use super::new_test_context;

#[tokio::test]
async fn should_init_tx_ring_buffer() {
    let ctx = new_test_context().await;
    let res = ctx.get_tx_from_ring_buffer(0).await.unwrap();
    assert!(res.is_some());
}

#[tokio::test]
async fn should_push_tx_to_ring_buffer() {
    let ctx = new_test_context().await;
    ctx.push_tx_to_ring_buffer(1, 1, 10).await.unwrap();

    assert!(ctx.get_tx_from_ring_buffer(1).await.unwrap().is_some());
}

#[tokio::test]
async fn should_persist_ring_buffer_tx_after_upgrade() {
    let ctx = new_test_context().await;
    ctx.push_tx_to_ring_buffer(1, 1, 10).await.unwrap();

    assert!(ctx.get_tx_from_ring_buffer(1).await.unwrap().is_some());

    super::upgrade_dummy_canister(&ctx).await.unwrap();

    assert!(ctx.get_tx_from_ring_buffer(0).await.unwrap().is_some());
    assert!(ctx.get_tx_from_ring_buffer(1).await.unwrap().is_some());
}
