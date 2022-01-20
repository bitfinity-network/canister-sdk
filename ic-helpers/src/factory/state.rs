/// This macro adds the following methods to the `$state` struct:
/// * `stable_save` - used to save the state to the stable storage
/// * `stable_restore` - used to load the state from the stable storage
/// * `reset` - used to replace the state in in-memory storage with the current one. This method
///   can be used in `init` method to set up the state.
///
/// It also provides `pre_upgrade` and `post_upgrade` functions.
///
/// IMPORTANT: This macro assumes that ths `$state` object is the only state used in the canister.
/// If this is not true, than this implementation cannot be used for state stable storage.
#[macro_export]
macro_rules! impl_factory_state_management {
    ( $state:ident, $bytecode:expr ) => {
        impl $state {
            pub fn stable_save(&self) {
                ::ic_cdk::storage::stable_save((self,)).unwrap();
            }

            pub fn stable_restore() {
                let (mut loaded,): (Self,) = ::ic_cdk::storage::stable_restore().unwrap();
                loaded.factory.restore($bytecode);
                loaded.reset();
            }

            pub fn reset(self) {
                let state = State::get();
                let mut state = state.borrow_mut();
                *state = self;
            }
        }

        #[::ic_cdk_macros::pre_upgrade]
        fn pre_upgrade() {
            $state::get().borrow().stable_save();
        }

        #[::ic_cdk_macros::post_upgrade]
        fn post_upgrade() {
            $state::stable_restore();
        }
    };
}
