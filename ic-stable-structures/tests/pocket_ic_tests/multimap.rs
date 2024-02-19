use super::with_pocket_ic_context;

#[test]
fn should_init_tx_multimap() {
    with_pocket_ic_context(|_, ctx| {
        assert!(ctx.get_tx_from_multimap(0)?.is_some());

        Ok(())
    })
    .unwrap();
}

#[test]
fn should_push_tx_to_multimap() {
    with_pocket_ic_context(|_, ctx| {
        ctx.insert_tx_to_multimap(1, 1, 10)?;

        assert!(ctx.get_tx_from_multimap(1).unwrap().is_some());

        Ok(())
    })
    .unwrap();
}

#[test]
fn should_persist_multimap_tx_after_upgrade() {
    with_pocket_ic_context(|_, ctx| {
        ctx.insert_tx_to_multimap(1, 1, 10)?;

        assert!(ctx.get_tx_from_multimap(1)?.is_some());

        super::upgrade_dummy_canister(ctx)?;

        assert!(ctx.get_tx_from_multimap(0)?.is_some());
        assert!(ctx.get_tx_from_multimap(1)?.is_some());

        Ok(())
    })
    .unwrap();
}
