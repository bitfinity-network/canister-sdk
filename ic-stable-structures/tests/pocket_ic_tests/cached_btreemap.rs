use super::with_pocket_ic_context;

#[test]
fn should_init_tx_cached_btreemap() {
    with_pocket_ic_context(|ctx| {
        assert!(ctx.get_tx_from_cached_btreemap(0)?.is_some());

        Ok(())
    })
    .unwrap();
}

#[test]
fn should_push_tx_to_cached_btreemap() {
    with_pocket_ic_context(|ctx| {
        // We saturate the cache to force eviction
        for i in 1..100 {
            ctx.insert_tx_to_cached_btreemap(i, i, 10 + i)?;
            assert!(ctx.get_tx_from_cached_btreemap(i as u64).unwrap().is_some());
        }

        for i in 1..100 {
            assert!(ctx.get_tx_from_cached_btreemap(i as u64).unwrap().is_some());
        }

        Ok(())
    })
    .unwrap();
}

#[test]
fn should_persist_cached_btreemap_tx_after_upgrade() {
    with_pocket_ic_context(|ctx| {
        ctx.insert_tx_to_cached_btreemap(1, 1, 10)?;

        assert!(ctx.get_tx_from_cached_btreemap(1)?.is_some());

        super::upgrade_dummy_canister(ctx)?;

        assert!(ctx.get_tx_from_cached_btreemap(1)?.is_some());

        Ok(())
    })
    .unwrap();
}
