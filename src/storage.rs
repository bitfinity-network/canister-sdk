use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::cell::RefCell;
use std::rc::Rc;

type StorageTree = BTreeMap<TypeId, Box<dyn Any>>;

static mut STORAGE: Option<Rc<RefCell<Storage>>> = None;

pub struct Storage(StorageTree);

impl Storage {
    fn load() -> Rc<RefCell<Self>> {
        unsafe {
            if STORAGE.is_none() {
                STORAGE = Some(Rc::new(RefCell::new(Self(StorageTree::new()))));
            }

            STORAGE.as_ref().unwrap().clone()
        }
    }

    fn get_mut<T: Sized + Default + 'static>(&mut self) -> &mut T {
        let type_id = std::any::TypeId::of::<T>();
        self.0
            .entry(type_id)
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("Unexpected value of invalid type.")
    }
}
