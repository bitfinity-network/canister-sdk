#[cfg(feature = "canbench-rs")]
mod benches {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use canbench_rs::bench;
    use ic_stable_structures::{
        default_ic_memory_manager, BTreeMapStructure, Bound, DefaultMemoryImpl, MemoryId,
        StableBTreeMap, Storable, VirtualMemory,
    };

    type Val = Vec<u8>;
    type VMem = VirtualMemory<DefaultMemoryImpl>;

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
    }

    const STABLE_MAP_MEM_ID: MemoryId = MemoryId::new(0);

    const VAL_SIZE: usize = 512;
    const INITIAL_ENTRIES_NUMBER: usize = 200_000;
    const OPS_ENTRIES_NUMBER: usize = 100_000;

    thread_local! {
        static HEAP_MAP: RefCell<HashMap<Key, Val>> = RefCell::new(HashMap::default());
        static STABLE_MAP: RefCell<StableBTreeMap<Key, Val, VMem>> = {
            let mem = default_ic_memory_manager().get(STABLE_MAP_MEM_ID);
            RefCell::new(StableBTreeMap::new(mem))
        };
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

    fn bench_map(map: &mut impl Map) {
        {
            let _scope = canbench_rs::bench_scope("init");
            init_map(map);
        }

        {
            let _scope = canbench_rs::bench_scope("get");
            get_items(map);
        }

        {
            let _scope = canbench_rs::bench_scope("replace");
            replace_items(map);
        }

        {
            let _scope = canbench_rs::bench_scope("remove");
            remove_items(map);
        }

        {
            let _scope = canbench_rs::bench_scope("add");
            add_items(map);
        }
    }

    #[bench]
    fn bench_heap_map() {
        ic_cdk::println!("HELLO");
        HEAP_MAP.with(|map| bench_map(&mut *map.borrow_mut()))
    }

    #[bench]
    fn bench_stable_map() {
        STABLE_MAP.with(|map| bench_map(&mut *map.borrow_mut()))
    }
}
