use ic_stable_structures::{
    BTreeMapStructure, MemoryId, MemoryManager, MemoryMappedFileMemory, StableBTreeMap, StableVec,
    VecStructure,
};
use tempfile::NamedTempFile;

#[test]
fn test_persistent_memory_mapped_file_memory() {
    let file = NamedTempFile::new().unwrap();
    let memory_resource =
        MemoryMappedFileMemory::new(file.path().to_str().unwrap().to_owned(), true).unwrap();
    let memory_manager = MemoryManager::init(memory_resource);

    let mut vec = StableVec::<u32, _>::new(memory_manager.get(MemoryId::new(0))).unwrap();
    vec.push(&1).unwrap();
    vec.push(&2).unwrap();
    vec.push(&3).unwrap();

    let mut map = StableBTreeMap::<u32, u64, _>::new(memory_manager.get(MemoryId::new(1)));
    map.insert(1, 2);
    map.insert(2, 3);
    map.insert(4, 5);
    drop(memory_manager);

    let memory_resource =
        MemoryMappedFileMemory::new(file.path().to_str().unwrap().to_owned(), true).unwrap();
    let memory_manager = MemoryManager::init(memory_resource);

    let vec = StableVec::<u32, _>::new(memory_manager.get(MemoryId::new(0))).unwrap();
    assert_eq!(vec.len(), 3);
    assert_eq!(vec.get(0), Some(1));
    assert_eq!(vec.get(1), Some(2));
    assert_eq!(vec.get(2), Some(3));

    let map = StableBTreeMap::<u32, u64, _>::new(memory_manager.get(MemoryId::new(1)));
    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&1), Some(2));
    assert_eq!(map.get(&2), Some(3));
    assert_eq!(map.get(&4), Some(5));
}
