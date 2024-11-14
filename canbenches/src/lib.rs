#[cfg(feature = "canbench-rs")]
mod benches {
    use std::collections::HashMap;
    use std::{cell::RefCell, rc::Rc};

    use canbench_rs::{bench, set_user_data};
    use ic_stable_structures::{
        BTreeMapStructure, Bound, DefaultMemoryImpl, IcMemoryManager, MemoryId, StableBTreeMap,
        Storable, VirtualMemory,
    };

    type Val = Vec<u8>;
    type VMem = VirtualMemory<Rc<DefaultMemoryImpl>>;

    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
    struct Key([u8; 32]);

    impl Storable for Key {
        fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
            (&self.0).into()
        }

        fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
            Self(bytes[..].try_into().unwrap())
        }

        const BOUND: Bound = Bound::Bounded {
            max_size: 32,
            is_fixed_size: true,
        };
    }

    trait Map {
        fn insert(&mut self, k: Key, v: Val);
        fn get(&self, k: &Key) -> Option<Val>;
        fn remove(&mut self, k: &Key) -> Option<Val>;
        fn iterate(&self, limit: usize, act: impl FnMut(&Key, &Val));
    }

    impl Map for HashMap<Key, Val> {
        fn insert(&mut self, k: Key, v: Val) {
            HashMap::insert(self, k, v);
        }

        fn get(&self, k: &Key) -> Option<Val> {
            HashMap::get(self, k).cloned()
        }

        fn remove(&mut self, k: &Key) -> Option<Val> {
            HashMap::remove(self, k)
        }

        fn iterate(&self, limit: usize, mut act: impl FnMut(&Key, &Val)) {
            for (k, v) in self.iter().take(limit) {
                act(k, v)
            }
        }
    }

    impl Map for StableBTreeMap<Key, Val, VMem> {
        fn insert(&mut self, k: Key, v: Val) {
            BTreeMapStructure::insert(self, k, v);
        }

        fn get(&self, k: &Key) -> Option<Val> {
            BTreeMapStructure::get(self, k)
        }

        fn remove(&mut self, k: &Key) -> Option<Val> {
            BTreeMapStructure::remove(self, k)
        }

        fn iterate(&self, limit: usize, mut act: impl FnMut(&Key, &Val)) {
            for (k, v) in self.iter().take(limit) {
                act(&k, &v)
            }
        }
    }

    impl<M: Map> Map for RefCell<M> {
        fn insert(&mut self, k: Key, v: Val) {
            self.borrow_mut().insert(k, v);
        }

        fn get(&self, k: &Key) -> Option<Val> {
            self.borrow_mut().get(k)
        }

        fn remove(&mut self, k: &Key) -> Option<Val> {
            self.borrow_mut().remove(k)
        }

        fn iterate(&self, limit: usize, act: impl FnMut(&Key, &Val)) {
            self.borrow().iterate(limit, act);
        }
    }

    const STABLE_MAP_MEM_ID: MemoryId = MemoryId::new(0);

    const VAL_SIZE: usize = 512;
    const INITIAL_ENTRIES_NUMBER: usize = 200_000;
    const OPS_ENTRIES_NUMBER: usize = 100_000;

    type MemoryManager = IcMemoryManager<Rc<DefaultMemoryImpl>>;

    thread_local! {
        static MEMORY: Rc<DefaultMemoryImpl> = Rc::default();
        static HEAP_MAP: RefCell<HashMap<Key, Val>> = RefCell::new(HashMap::default());
        static STABLE_MAP: RefCell<StableBTreeMap<Key, Val, VMem>> = {
            let mem = get_memory_manager().get(STABLE_MAP_MEM_ID);
            RefCell::new(StableBTreeMap::new(mem))
        };
    }

    fn get_memory_manager() -> MemoryManager {
        IcMemoryManager::init(MEMORY.with(|m| m.clone()))
    }

    fn get_memory_access_report() -> String {
        MEMORY.with(|m| m.stats().to_string())
    }

    fn reset_memory_accesses() {
        MEMORY.with(|m| m.reset_stats())
    }

    fn key(n: u64) -> Key {
        let mut buf = [0u8; 32];
        buf[..8].copy_from_slice(&n.to_be_bytes());
        Key(buf)
    }

    fn val(n: u64) -> Val {
        let mut buf = vec![0u8; VAL_SIZE];
        buf[..8].copy_from_slice(&n.to_be_bytes());
        buf
    }

    fn init_map(map: &mut impl Map) {
        for i in 0..INITIAL_ENTRIES_NUMBER {
            map.insert(key(i as _), val(i as _));
        }
    }

    fn get_items(map: &mut impl Map) {
        let mut idx = 7919;
        for _ in 0..OPS_ENTRIES_NUMBER {
            let val = map.get(&key(idx)).unwrap();
            assert_eq!(val[0..8], idx.to_be_bytes()[0..8]); // required to prevent optimization
            idx = idx.wrapping_add(idx) % INITIAL_ENTRIES_NUMBER as u64;
        }
    }

    fn replace_items(map: &mut impl Map) {
        let mut idx = 7919;
        for _ in 0..OPS_ENTRIES_NUMBER {
            map.insert(key(idx), val(idx));
            idx = idx.wrapping_add(idx) % INITIAL_ENTRIES_NUMBER as u64;
        }
    }

    fn remove_items(map: &mut impl Map) {
        let mut idx = 7919;
        for _ in 0..OPS_ENTRIES_NUMBER {
            map.remove(&key(idx));
            idx = idx.wrapping_add(idx) % INITIAL_ENTRIES_NUMBER as u64;
        }
    }

    fn add_items(map: &mut impl Map) {
        let mut idx = INITIAL_ENTRIES_NUMBER as u64 + 7919;
        let end_number = INITIAL_ENTRIES_NUMBER + OPS_ENTRIES_NUMBER;
        for _ in INITIAL_ENTRIES_NUMBER..end_number {
            map.insert(key(idx), val(idx));
            idx = idx.wrapping_add(idx);
        }
    }

    fn iter_items(map: &mut impl Map) {
        map.iterate(OPS_ENTRIES_NUMBER, |k, v| {
            assert_eq!(k.0[0..8], v[0..8]); // required to prevent optimization
        });
    }

    fn bench_map(map: &mut impl Map) {
        {
            let _scope = canbench_rs::bench_scope("init");
            reset_memory_accesses();
            init_map(map);
            set_user_data(Some(get_memory_access_report()));
        }

        {
            let _scope = canbench_rs::bench_scope("get");
            reset_memory_accesses();
            get_items(map);
            set_user_data(Some(get_memory_access_report()));
        }

        {
            let _scope = canbench_rs::bench_scope("replace");
            reset_memory_accesses();
            replace_items(map);
            set_user_data(Some(get_memory_access_report()));
        }

        {
            let _scope = canbench_rs::bench_scope("iterate");
            reset_memory_accesses();
            iter_items(map);
            set_user_data(Some(get_memory_access_report()));
        }

        {
            let _scope = canbench_rs::bench_scope("remove");
            reset_memory_accesses();
            remove_items(map);
            set_user_data(Some(get_memory_access_report()));
        }

        {
            let _scope = canbench_rs::bench_scope("add");
            reset_memory_accesses();
            add_items(map);
            set_user_data(Some(get_memory_access_report()));
        }

        set_user_data(None);
    }

    #[bench]
    fn bench_heap_map() {
        HEAP_MAP.with(|map| bench_map(&mut *map.borrow_mut()));
    }

    #[bench]
    fn bench_stable_map() {
        STABLE_MAP.with(|map| bench_map(&mut *map.borrow_mut()))
    }
}
