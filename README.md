# canister-sdk

SDK for writing and testing canisters for the Internet Computer in Rust. This repo includes a few crates that help to
simplify the tricky aspects of IC canisters development:

- [simplifying and testing of inter-canister communications](#inter-canister-calls-and-testing)
- [allowing dependencies between canisters](#dependencies-between-canisters)
- [canisters composition](#canister-traits-and-composition) (combining APIs from different crates into a single canister)
- [in-memory state management](#in-memory-storage)
- [versioning of the state for canister upgrades](#versioned-state)
- [collect canister metrics and define your own](#ic-metrics)

This project builds on top of `ic-cdk` and `ic-kit` crates. It is not intended to replace them, but adds some types and
macros to simplify things that are not dealt with by those crates.

# Crates

## canister-sdk

A wrapper crate among all of the canisters in this repository with all of the necessary re-exports. It some features for exporting factory and auction traits as well as their APIs.

Note: Currently we require each canister to have its own `export_api` feature that will export the wasm definitions of the canister (and hide them if this canister is used as a dependency). This is not the case with trait canisters as rust features are additive even in transitive dependencies we're forced to introduce different `export_api` features for different traits in this repo (`auction_api` and `factory_api` respectively). The best approach will be to use `canister_sdk` dependency with dependent features that will trigger the necessary APIs transitively, e.g.

```yaml
[package]
edition = "2021"
name = "awesome-canister"
version = "0.1.0"

[features]
default = []
export_api = ["canister-sdk/auction_api"]
auction = ["canister-sdk/auction"]

[dependencies]
canister-sdk = { git = "https://github.com/infinity-swap/canister-sdk", package = "canister-sdk" }
```

## ic-exports

All of the ic dependencies re-exported in one crate that simplifies their updating process.

## ic-canister

This crate introduces a framework to write easily testable canisters, including testing inter-canister calls,
as well as a way to compose canister APIs using rust traits. There are few examples below, but for the details
you can check out the [crate documentation](./ic-canister/ic-canister/src/lib.rs).

### Inter-canister calls and testing

For example, you can have a canister with simple API:

```rust
#[derive(Clone, Canister)]
struct MyCanister {
    #[id]
    principal: Principal,

    #[state]
    state: Rc<RefCell<MyCanisterState>>,
}

impl MyCanister {
    #[query]
    fn get_counter(&self) -> u64 {
        self.state.borrow().counter
    }

    #[update]
    fn add(&self, value: u64) {
        self.state.borrow_mut().counter += value;
    }
}
```

Now instead of using `ic_cdk::api::call` to query these APIs from another canister doing manual
serialization/deserialization:

```rust

impl MyOtherCanister {
    #[update]
    async fn increment_get(&self, id: Principal) -> u64 {
        let my_canister = MyCanister::from_principal(id);
        canister_call!(my_canister.add(1), ()).await.unwrap();
        let updated_value = canister_call!(my_canister.get_counter(), u64).await.unwrap();

        updated_value
    }
}
```

Now, if you want to test the `increment_get` method of you canister, all you need to do is to write a unit test:

```rust
#[tokio::test]
async fn test_increment_get() {
    let my_canister = MyCanister::init_instance();
    let my_other_canister = MyOtherCanister::init_instance();

    assert_eq!(my_other_canister.increment_get(my_canister.principal()), 1);
    assert_eq!(my_other_canister.increment_get(my_canister.principal()), 2);

}
```

Even though the canisters use statics internally to store the canister state, the tests can initialize multiple instances of
canisters with `init_instance` method, and each one of them will have a separate state.

### Canister traits and composition

It is also possible to write a trait to store some part of the canister API that can be reused in different canisters. This also allows to compose a canister from different
traits just by implementing the needed traits for your canister Struct:

```rust
impl Factory for MyCanister {
    fn state(&self) -> Rc<RefCell<FactoryState>> {
        <Self as Factory>::FactoryStateState::get()
    }
}

impl OtherCanisterTrait for MyCanister {
    // impl
}
```

For other examples you can look into the [tests](https://github.com/infinity-swap/canister-sdk/blob/f835312e13b567ca2cb1d75cc3a1647da5d41204/ic-canister/tests/canister_a/src/lib.rs#L21-L46)

### Dependencies between canisters

If you use `ic-cdk` to create your canister's API's, you cannot simply use a canister as a rust dependency for another
canister, because all the API's of that dependency will not be included into the canister you're implementing. If you create a canister using `canister-sdk`, don't forget to add a `export_api` feature to the dependency so that it will export all of the wasm from the canister trait definition (more specifically from one that is auto-generated by `generate_exports!` macro).

## ic-factory

The "generic" trait API for the factory canisters. The `Factory` canister trait can be used to simplify writing factory logic for the canisters you implement. It also provides a convenient way to upgrade all the canisters that the factory is set to be controller of.

## ic-helpers

Some helpers and wrappers over principals we use on day-to-day basis for ledger and management canister calls.

## ic-storage

Introduces traits `IcStorage` and `Versioned` for in-memory and stable storage management respectively.

### In-memory storage

In the past, the `ic-cdk` crate provided methods in the `ic_cdk::storage` module to store and get structs from the canister's memory, but they were removed in version `0.5.0` (for a good reason). The recommended way to store the data in the canister
memory is to use `thread_local` storage with `RefCell` controlling access to the struct.

The `ic_storage::IcStorage` deriving macro does exactly that, but saving you some boilerplate. Using it is quite
straightforward:

```rust
use ic_storage::IcStorage;

#[derive(IcStorage, Default)]
struct MyCanisterState {
    value: u32,
}

let local_state = MyCanisterState::get();
assert_eq!(local_state.borrow().value, 0);
local_state.borrow_mut().value = 42;
assert_eq!(local_state.borrow().value, 42);
```

It also allows having generic state structures. For detailed information, check out the [crate level documentation](./ic-storage/src/lib.rs).

### Versioned state

The `ic_storage::stable` module introduces `Versioned` trait that allows transparent upgrades for the state during
canister upgrades (even over several versions of state at once). When using this trait, the state structure can
be serialized into the stable storage using `ic_storage::stable::write` method. Then after the upgrade, simply use
`ic_storage::stable::read::<NewStateType>()`. This will read the serialized previous version of the state, check its
version and run the upgrade methods until the current version of the type (the `NewStateType` struct) is reached.

Check out the [module level documentation](./ic-storage/src/stable.rs) for more details.

### Canister state and upgrades

When using `Canister` deriving macro, the fields that are marked with `#[state]` attribute are all preserved over
canister upgrades. This is done using `Versioned` trait. This means that at this moment you can have only one `#[state]` in a canister. If the state type is changed, the new state must have the previous state type as its `Versioned::Previous` type. The `Canister` deriving macro takes care of generating the `pre_upgrade` and `post_upgrade` functions and updating the state to the new type when needed.

If a canister needs to have a state that is not preserved during the upgrade process (like caches or some other
temporary data), `#[state(stable_store = false)]` can be used in addition to the `#[state]` field. Any number of
non-stable state fields can be added to a canister.

## ic-metrics

Metrics trait that the canister can implement to store a history of necessary metrics (stored in [MetricsData](https://github.com/infinity-swap/canister-sdk/blob/f835312e13b567ca2cb1d75cc3a1647da5d41204/ic-metrics/src/map.rs#L14-L18)) for the canister that also
allows to overwrite the `update_metrics` call to store custom metrics for a canister. For an example you can refer to the [tests](https://github.com/infinity-swap/canister-sdk/blob/main/ic-canister/tests/canister-c/src/lib.rs#L43-L49).
