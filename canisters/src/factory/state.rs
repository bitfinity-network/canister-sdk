use crate::factory::Factory;
use candid::types::internal::Type;
use candid::types::Serializer;
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::error::Error;
use std::hash::Hash;
use std::marker::PhantomData;

#[derive(CandidType, Clone, Serialize, Deserialize)]
#[serde(
    bound = "K: Serialize, for<'a> K: Deserialize<'a>, S: Serialize, for<'a> S: Deserialize<'a>"
)]
pub struct State<
    K: 'static + Hash + Eq,
    S: 'static + Default,
    W: 'static + DataProvider + CandidType,
> where
    K: Serialize + for<'a> Deserialize<'a>,
    S: Serialize + for<'a> Deserialize<'a>,
{
    pub admin: Principal,
    pub settings: S,
    pub factory: Factory<K>,
    phantom: PhantomDataExt<W>,
}

impl<K: 'static + Hash + Eq, S: 'static + Default, W: 'static + DataProvider + CandidType> Default
    for State<K, S, W>
where
    K: Serialize + for<'a> Deserialize<'a>,
    S: Serialize + for<'a> Deserialize<'a>,
{
    fn default() -> Self {
        Self {
            admin: Principal::anonymous(),
            settings: S::default(),
            factory: Factory::new(W::wasm_module()),
            phantom: PhantomDataExt(PhantomData),
        }
    }
}

impl<K: 'static + Hash + Eq, S: 'static + Default, W: 'static + DataProvider + CandidType>
    State<K, S, W>
where
    K: Serialize + for<'a> Deserialize<'a>,
    S: Serialize + for<'a> Deserialize<'a>,
{
    /// Returns a reference to current state.
    /// If state does not exists, a new instance of its default is created.
    pub fn get() -> &'static mut Self {
        W::state().downcast_mut::<Self>().unwrap()
    }

    /// Returns bytecode of wasm module.
    pub fn wasm() -> &'static [u8] {
        W::wasm_module()
    }

    /// Stores current state to stable memory.
    pub fn save() -> Result<(), Box<dyn Error>> {
        let buf: Vec<u8> = Self::get().try_into()?;
        Ok(ic_cdk::storage::stable_save((buf,))?)
    }

    /// Restores a state from stable memory and updates wasm module checksums if needed.
    pub fn restore() -> Result<(), Box<dyn Error>> {
        let (buf,) = ic_cdk::storage::stable_restore::<(Vec<u8>,)>()?;
        let mut state: Self = buf.try_into()?;
        state.factory.restore(Self::wasm());
        *Self::get() = state;
        Ok(())
    }
}

impl<K: 'static + Hash + Eq, S: 'static + Default, W: 'static + DataProvider + CandidType>
    TryFrom<Vec<u8>> for State<K, S, W>
where
    K: Serialize + for<'a> Deserialize<'a>,
    S: Serialize + for<'a> Deserialize<'a>,
{
    type Error = Box<dyn Error>;

    fn try_from(buf: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(serde_json::from_slice(buf.as_slice())?)
    }
}

impl<K: 'static + Hash + Eq, S: 'static + Default, W: 'static + DataProvider + CandidType>
    TryFrom<&mut State<K, S, W>> for Vec<u8>
where
    K: Serialize + for<'a> Deserialize<'a>,
    S: Serialize + for<'a> Deserialize<'a>,
{
    type Error = Box<dyn Error>;

    fn try_from(state: &mut State<K, S, W>) -> Result<Self, Self::Error> {
        Ok(serde_json::to_vec(state)?)
    }
}

pub trait DataProvider {
    fn wasm_module() -> &'static [u8];
    fn state() -> &'static mut dyn Any;
}

#[derive(Clone, Serialize, Deserialize)]
struct PhantomDataExt<T>(PhantomData<T>);

impl<T> CandidType for PhantomDataExt<T> {
    fn _ty() -> Type {
        Type::Null
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_null(())
    }
}

#[macro_export]
macro_rules! init_state {
    ( $name:ident, $key:ident, $settings:ident, $wasm:expr ) => {
        pub type $name = ic_helpers::factory::State<$key, $settings, Data>;
        static mut STATE: Option<$name> = None;

        #[export_name = "canister_pre_upgrade"]
        pub fn pre_upgrade() {
            ic_cdk::print("saving state to stable memory");
            $name::save().unwrap();
        }

        #[export_name = "canister_post_upgrade"]
        pub fn post_upgrade() {
            ic_cdk::print("restoring state from stable memory");
            $name::restore().unwrap();
        }

        #[derive(candid::CandidType, Clone, serde::Serialize, serde::Deserialize)]
        pub struct Data;

        impl ic_helpers::factory::DataProvider for Data {
            fn wasm_module() -> &'static [u8] {
                include_bytes!($wasm)
            }

            fn state() -> &'static mut dyn std::any::Any {
                unsafe {
                    if STATE.is_none() {
                        STATE = Some($name::default());
                    }
                    STATE.as_mut().unwrap()
                }
            }
        }
    };
}
