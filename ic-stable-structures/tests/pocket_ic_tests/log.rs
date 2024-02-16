use super::with_pocket_ic_context;

#[test]
fn should_init_tx_log() {
    with_pocket_ic_context(|ctx| {
        let res = ctx.get_tx_from_log(0)?;
        assert!(res.is_some());
        Ok(())
    })
    .unwrap();
}

#[test]
fn should_push_tx_to_log() {
    with_pocket_ic_context(|ctx| {
        ctx.push_tx_to_log(1, 1, 10)?;

        assert!(ctx.get_tx_from_log(1)?.is_some());

        Ok(())
    })
    .unwrap();
}

#[test]
fn should_persist_log_tx_after_upgrade() {
    with_pocket_ic_context(|ctx| {
        ctx.push_tx_to_log(1, 1, 10)?;

        assert!(ctx.get_tx_from_log(1)?.is_some());

        super::upgrade_dummy_canister(ctx)?;

        assert!(ctx.get_tx_from_log(0)?.is_some());
        assert!(ctx.get_tx_from_log(1)?.is_some());

        Ok(())
    })
    .unwrap();
}
