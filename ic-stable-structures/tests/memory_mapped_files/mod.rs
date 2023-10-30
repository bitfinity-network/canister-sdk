use std::sync::Arc;

use ic_stable_structures::{
    BTreeMapStructure, IcMemoryManager, MemoryId, MemoryManager, MemoryMappedFileMemory,
    MemoryMappedFileMemoryManager, StableBTreeMap, StableVec, VecStructure,
};
use parking_lot::Mutex;
use tempfile::{NamedTempFile, TempDir};

#[test]
fn test_persistent_memory_mapped_file_memory() {
    let file = NamedTempFile::new().unwrap();
    let memory_resource =
        MemoryMappedFileMemory::new(file.path().to_str().unwrap().to_owned(), true).unwrap();
    let memory_manager = IcMemoryManager::init(memory_resource);

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
    let memory_manager = IcMemoryManager::init(memory_resource);

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

#[test]
fn test_memory_mapped_file_memory_manager() {
    let base_dir = TempDir::new().unwrap();
    let base_path = base_dir.into_path();
    let expected_file_0_path = base_path.join("0");
    let expected_file_1_path = base_path.join("1");

    let memory_manager = MemoryMappedFileMemoryManager::new(base_path.clone(), true);

    assert!(!expected_file_0_path.exists());
    assert!(!expected_file_1_path.exists());

    let mut vec = StableVec::<u32, _>::new(memory_manager.get(0)).unwrap();
    vec.push(&1).unwrap();
    vec.push(&2).unwrap();
    vec.push(&3).unwrap();

    assert!(expected_file_0_path.exists());
    assert!(!expected_file_1_path.exists());

    let mut map = StableBTreeMap::<u32, u64, _>::new(memory_manager.get(1));
    map.insert(1, 2);
    map.insert(2, 3);
    map.insert(4, 5);

    assert!(expected_file_0_path.exists());
    assert!(expected_file_1_path.exists());

    drop(memory_manager);

    assert!(expected_file_0_path.exists());
    assert!(expected_file_1_path.exists());

    let memory_manager = MemoryMappedFileMemoryManager::new(base_path, true);

    let vec = StableVec::<u32, _>::new(memory_manager.get(0)).unwrap();
    assert_eq!(vec.len(), 3);
    assert_eq!(vec.get(0), Some(1));
    assert_eq!(vec.get(1), Some(2));
    assert_eq!(vec.get(2), Some(3));

    let map = StableBTreeMap::<u32, u64, _>::new(memory_manager.get(1));
    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&1), Some(2));
    assert_eq!(map.get(&2), Some(3));
    assert_eq!(map.get(&4), Some(5));
}

#[test]
fn test_memory_mapped_file_memory_manager_is_send() {
    let base_dir = TempDir::new().unwrap();
    let base_path = base_dir.into_path();

    let memory_manager = MemoryMappedFileMemoryManager::new(base_path.clone(), true);

    let vec = StableVec::<u32, _>::new(memory_manager.get(0)).unwrap();
    let arc_state = Arc::new(Mutex::new(vec));

    let mut handles = vec![];

    for x in 0..10 {
        let arc_state_clone = arc_state.clone();
        let handler = std::thread::spawn(move || {
            arc_state_clone.lock().push(&x).unwrap();
        });
        handles.push(handler);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(10, arc_state.lock().len());
}

#[test]
fn test_memory_mapped_file_memory_manager_saves_copy() {
    let base_dir = TempDir::new().unwrap();
    let base_path = base_dir.into_path();
    let expected_file_0_path = base_path.join("0");
    let expected_file_1_path = base_path.join("1");

    let memory_manager = MemoryMappedFileMemoryManager::new(base_path.clone(), true);

    assert!(!expected_file_0_path.exists());
    assert!(!expected_file_1_path.exists());

    let mut vec = StableVec::<u32, _>::new(memory_manager.get(0)).unwrap();
    vec.push(&1).unwrap();
    vec.push(&2).unwrap();
    vec.push(&3).unwrap();

    assert!(expected_file_0_path.exists());
    assert!(!expected_file_1_path.exists());

    let mut map = StableBTreeMap::<u32, u64, _>::new(memory_manager.get(1));
    map.insert(1, 2);
    map.insert(2, 3);
    map.insert(4, 5);

    let backup_dir = TempDir::new().unwrap();
    let backup_path = backup_dir.into_path();
    memory_manager
        .save_copies_to(backup_path.clone())
        .unwrap();

    drop(memory_manager);
    let memory_manager = MemoryMappedFileMemoryManager::new(backup_path, true);

    let vec = StableVec::<u32, _>::new(memory_manager.get(0)).unwrap();
    assert_eq!(vec.get(0), Some(1));
    assert_eq!(vec.get(1), Some(2));
    assert_eq!(vec.get(2), Some(3));

    let map = StableBTreeMap::<u32, u64, _>::new(memory_manager.get(1));
    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&1), Some(2));
    assert_eq!(map.get(&2), Some(3));
    assert_eq!(map.get(&4), Some(5));
}
