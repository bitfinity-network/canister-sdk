#[derive(Default)]
pub struct Manager(HashMap<Principal, MemoryManager>);

impl Manager {
    pub fn get(&mut self, memory_id: MemoryId) -> Memory {
        let canister_id = ic::id();
        self.0
            .entry(canister_id)
            .or_insert_with(|| MemoryManager::init(DefaultMemoryImpl::default()))
            .get(memory_id)
    }
}