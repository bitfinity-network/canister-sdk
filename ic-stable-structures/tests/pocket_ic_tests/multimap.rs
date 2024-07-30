use super::new_test_context;

#[tokio::test]
async fn should_init_tx_multimap() {
    let ctx = new_test_context().await;
    assert!(ctx.get_tx_from_multimap(0).await.unwrap().is_some());
}

#[tokio::test]
async fn should_push_tx_to_multimap() {
    let ctx = new_test_context().await;
    ctx.insert_tx_to_multimap(1, 1, 10).await.unwrap();

    assert!(ctx.get_tx_from_multimap(1).await.unwrap().is_some());
}

#[tokio::test]
async fn should_persist_multimap_tx_after_upgrade() {
    let ctx = new_test_context().await;
    ctx.insert_tx_to_multimap(1, 1, 10).await.unwrap();

    assert!(ctx.get_tx_from_multimap(1).await.unwrap().is_some());

    super::upgrade_dummy_canister(&ctx).await.unwrap();

    assert!(ctx.get_tx_from_multimap(0).await.unwrap().is_some());
    assert!(ctx.get_tx_from_multimap(1).await.unwrap().is_some());
}
