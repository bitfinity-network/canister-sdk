use std::marker::PhantomData;

// use candid::{CandidType, Deserialize};

use super::VirtualMemory;


pub struct StableVec<T, const INDEX: u8>(PhantomData<T>);


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insert() {
        let mut vec = StableVec::<_, 0>::new();
        assert_eq!(expected, actual);
    }
}
