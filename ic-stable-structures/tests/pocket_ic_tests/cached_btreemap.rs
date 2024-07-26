use super::new_test_context;

#[tokio::test]
async fn should_init_tx_cached_btreemap() {
let ctx = new_test_context().await;
        assert!(ctx.get_tx_from_cached_btreemap(0).await.unwrap().is_some());


}

#[tokio::test]
async fn should_push_tx_to_cached_btreemap() {
let ctx = new_test_context().await;
        // We saturate the cache to force eviction
        for i in 1..100 {
            ctx.insert_tx_to_cached_btreemap(i, i, 10 + i).await.unwrap();
            assert!(ctx.get_tx_from_cached_btreemap(i as u64).await.unwrap().is_some());
        }

        for i in 1..100 {
            assert!(ctx.get_tx_from_cached_btreemap(i as u64).await.unwrap().is_some());
        }


}

#[tokio::test]
async fn should_persist_cached_btreemap_tx_after_upgrade() {
let ctx = new_test_context().await;
        ctx.insert_tx_to_cached_btreemap(1, 1, 10).await.unwrap();

        assert!(ctx.get_tx_from_cached_btreemap(1).await.unwrap().is_some());

        super::upgrade_dummy_canister(&ctx).await.unwrap();

        assert!(ctx.get_tx_from_cached_btreemap(1).await.unwrap().is_some());


}
