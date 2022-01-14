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
    let storage = TestStorage::get();
    assert_eq!(storage.borrow().val, 0);
}

#[test]
fn generic_storage_derive() {
    let storage = GenericStorage::<u128>::get();
    assert_eq!(storage.borrow().val, 0);
}
