# canister-sdk

An SDK for writing and testing canisters for the Internet Computer in Rust. This repo includes a few crates that help to
simplify the tricky aspects of IC canisters development:


* [simplifying and testing of inter-canister communications](#inter-canister-calls-and-testing)
* [allowing dependencies between canisters](#dependencies-between-canisters)
* [canisters composition](#canister-traits-and-composition) (combining APIs from different crates into a single canister)
* [in-memory state management](#in-memory-storage)
* [versioning of the state for canister upgrades](#versioned-state)

This project builds on top of `ic-cdk` and `ic-kit` crates. It is not intended to replace them, but adds some types and
macros to simplify things that are not dealt with by those crates.

# Crates


## ic-canister

This crate introduces a framework to write easily testable canisters, including testing inter-canister communications,
as well as a way to compose a canister APIs using rust traits. There are a few examples bellow, but for the details
check out the [crate documentation](./ic-canister/ic-canister/src/lib.rs).

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

Even though the canisters use statics internally to store the state, the tests can initialize multiple instances of
canisters with `init_instance` method, and each one of them will have a separate state.

### Canister state and upgrades

When using `Canister` derive macro, the fields that are marked with `#[state]` attribute are all preserved over
canister upgrades. This is done using `Versioned` trait. This means that at this moment you can have only one `#[state]`
in a canister. If the state type is changed, the new state must have the previous state type as its `Versioned::Previous`
type. The `Canister` derive macro take care of generating the `pre_upgrade` and `post_upgrade` functions and updating
the state to the new type when needed.

If a canister needs to have a state that is not preserved during the upgrade process (like caches or some other
temporary data), `#[state(stable_store = false)]` can be used in addition to the `#[state]` field. Any number of 
non-stable state fields can be added to a canister.

### Canister traits and composition

It is also possible to write a canister trait to store some part of API to be reused in different canisters. You can
find an example of such a trait in the `ic_factory::api` module. This also allows to compose a canister from different
traits just by implementing the needed traits for your canister struct:

```rust
impl Factory for MyCanister {
    fn state(&self) -> Rc<RefCell<FactoryState>> {
        self.factory_state.clone()
    }
}

impl OtherCanisterTrait for MyCanister {
    // impl
}
```

**NOTE**: this part of the SDK is still work in progress, so some parts of it are subject to change

### Dependencies between canisters

If you use `ic-cdk` to create your canister's `API`s, you cannot simply use a canister as a rust dependency for another
canister, because all the `API`s of the dependency will also be included into the dependent canister. If you create
a canister using `canister-sdk`, just add a `no_api` feature to your canister. When this feature is enabled on a
dependency, the API of the dependency will not be exported white the rest of logic and types can be used in you crate.

## ic-factory

Base of a canister factory canister. The `Factory` canister trait can be used to simply write a canister factory for 
canisters of you type. It also provides a convenient way to upgrade all the canister that the factory is set to manage.

## ic-helpers

This crate contains some types, helper functions and re-exports for `ic` repo. These are just used for convenience and
the overall structure of the crate is not finalized (and some stuff is outdated and will be removed in the future).


## ic-storage

Introduces traits `IcStorage` and `Versioned` for in-memory and stable storage management respectively.

### In-memory storage

Int he past, the `ic-cdk` crate provided methods in the `ic_cdk::storage` module to store and get structs from the canister's memory, but they were removed in version `0.5.0` (for a good reason). The recommended way to store the data in the canister
memory is to use `thread_local` storage with `RefCell` controlling access to the struct.

The `ic_storage::IcStorage` derive macro does exactly, but saving you some boilerplate. Using it is quite
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

The `ic_storage::stable` module introduces `Versioned` trait that allows transparent upgrades for you state on
canister upgrades (event over several versions of state at once). When using this trait, the state structure can
be serialized into the stable storage using `ic_storage::stable::write` method. Then after the upgrade, simply use
`ic_storage::stable::read::<NewStateType>()`. This will read the serialized previous version of the state, check its
version and run the upgrade methods until the current version of the type (the `NewStateType` struct) is reached.

Check out the [module level documentation](./ic-storage/src/stable.rs) for more details.
