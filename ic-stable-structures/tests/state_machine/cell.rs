use super::with_state_machine_context;

#[test]
fn should_init_tx_cell() {
    with_state_machine_context(|_, ctx| {
        assert_eq!(ctx.get_tx_from_cell()?.from, 0);

        Ok(())
    })
    .unwrap();
}

//#[test]
fn should_push_tx_to_cell() {
    with_state_machine_context(|_, ctx| {
        ctx.insert_tx_to_cell(1, 1, 10)?;

        assert_eq!(ctx.get_tx_from_cell()?.from, 1);

        Ok(())
    })
    .unwrap();
}

//#[test]
fn should_persist_cell_tx_after_upgrade() {
    with_state_machine_context(|_, ctx| {
        ctx.insert_tx_to_cell(1, 1, 10)?;

        assert_eq!(ctx.get_tx_from_cell()?.from, 1);

        super::upgrade_dummy_canister(ctx)?;

        assert_eq!(ctx.get_tx_from_cell()?.from, 1);

        Ok(())
    })
    .unwrap();
}
