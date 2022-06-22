use ic_canister::ic_kit::MockContext;
use ic_storage::{generic_derive, IcStorage};

#[derive(IcStorage, Default)]
struct TestStorage {
    val: u32,
}

#[derive(Default)]
struct GenericStorage<T> {
    val: T,
}

generic_derive!(GenericStorage<u128>);

#[test]
fn storage_derive_macro() {
    MockContext::new().inject();

    let storage = TestStorage::get();
    assert_eq!(storage.borrow().val, 0);
}

#[test]
fn generic_storage_derive() {
    MockContext::new().inject();

    let storage = GenericStorage::<u128>::get();
    assert_eq!(storage.borrow().val, 0);
}
