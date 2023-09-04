use criterion::{criterion_group, criterion_main, Criterion};
use ic_stable_structures::*;
use rand::distributions::{Alphanumeric, DistString};
use types::StringValue;

fn multimap_benchmark(c: &mut Criterion) {
    let mut map = StableMultimap::new(MemoryId::new(0));

    let key1_count = 100u64;
    let key2_count = 100u64;

    c.bench_function("multimap_benchmark", |b| {
        b.iter(|| {
            for k1 in 0..key1_count {
                for k2 in 0..key2_count {
                    let value: u128 = rand::random();
                    map.insert(&k1, &k2, &value);
                }
            }
            for k1 in 0..key1_count {
                for k2 in 0..key2_count {
                    assert!(map.get(&k1, &k2).is_some())
                }
            }
        })
    });
}

fn unboundedmap_benchmark(c: &mut Criterion) {
    let mut map = StableUnboundedMap::new(MemoryId::new(1));

    let key1_count = 10000u64;

    c.bench_function("unboundedmap_benchmark", |b| {
        b.iter(|| {
            for k1 in 0..key1_count {
                let value = StringValue(Alphanumeric.sample_string(&mut rand::thread_rng(), 128));
                map.insert(&k1, &value);
            }
            for k1 in 0..key1_count {
                assert!(map.get(&k1).is_some())
            }
        })
    });
}

criterion_group!(benches, multimap_benchmark, unboundedmap_benchmark);
criterion_main!(benches);

mod types {

    use std::borrow::Cow;

    use ic_exports::stable_structures::Storable;
    use ic_stable_structures::{ChunkSize, SlicedStorable};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StringValue(pub String);

    impl Storable for StringValue {
        fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
            self.0.to_bytes()
        }

        fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
            Self(String::from_bytes(bytes))
        }
    }

    impl SlicedStorable for StringValue {
        const CHUNK_SIZE: ChunkSize = 64;
    }
}
