use super::with_state_machine_context;

#[test]
fn should_init_tx_map() {
    with_state_machine_context(|_, ctx| {
        assert!(ctx.get_tx_from_map(0)?.is_some());

        Ok(())
    })
    .unwrap();
}

#[test]
fn should_push_tx_to_map() {
    with_state_machine_context(|_, ctx| {
        ctx.insert_tx_to_map(1, 1, 10)?;

        assert!(ctx.get_tx_from_map(1).unwrap().is_some());

        Ok(())
    })
    .unwrap();
}

#[test]
fn should_persist_map_tx_after_upgrade() {
    with_state_machine_context(|_, ctx| {
        ctx.insert_tx_to_map(1, 1, 10)?;

        assert!(ctx.get_tx_from_map(1)?.is_some());

        super::upgrade_dummy_canister(ctx)?;

        assert!(ctx.get_tx_from_map(0)?.is_some());
        assert!(ctx.get_tx_from_map(1)?.is_some());

        Ok(())
    })
    .unwrap();
}
