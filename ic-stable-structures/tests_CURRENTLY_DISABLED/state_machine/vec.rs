use super::with_state_machine_context;

#[test]
fn should_init_tx_vec() {
    with_state_machine_context(|_, ctx| {
        let res = ctx.get_tx_from_vec(0)?;
        assert!(res.is_some());
        Ok(())
    })
    .unwrap();
}

#[test]
fn should_push_tx_to_vec() {
    with_state_machine_context(|_, ctx| {
        ctx.push_tx_to_vec(1, 1, 10)?;

        assert!(ctx.get_tx_from_vec(1)?.is_some());

        Ok(())
    })
    .unwrap();
}

#[test]
fn should_persist_vec_tx_after_upgrade() {
    with_state_machine_context(|_, ctx| {
        ctx.push_tx_to_vec(1, 1, 10)?;

        assert!(ctx.get_tx_from_vec(1)?.is_some());

        super::upgrade_dummy_canister(ctx)?;

        assert!(ctx.get_tx_from_vec(0)?.is_some());
        assert!(ctx.get_tx_from_vec(1)?.is_some());

        Ok(())
    })
    .unwrap();
}
