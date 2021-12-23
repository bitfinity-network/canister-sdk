use ic_storage::IcStorage;

#[derive(IcStorage, Default)]
struct TestStorage {
    val: u32,
}

#[test]
fn test_storage_derive_macro() {
    let storage = TestStorage::get();
    assert_eq!(storage.borrow().val, 0);
}