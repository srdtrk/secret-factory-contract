use std::{any::type_name, marker::PhantomData};

use cosmwasm_std::{ReadonlyStorage, StdResult, StdError, Storage};
use secret_toolkit_serialization::{Serde};
use serde::{Serialize, de::DeserializeOwned};

// ---------------------------- Explicit Storage ------------------------------ //
// This serves as a replacement to Singleton

pub struct ExplicitStorage<'a, T: Serialize + DeserializeOwned, Ser: Serde> {
    pub storage_key: &'a [u8],
    pub item_type: PhantomData<T>,
    pub serialization_type: PhantomData<Ser>,
}

impl<'a, T: Serialize + DeserializeOwned, Ser: Serde> ExplicitStorage<'a, T, Ser> {
    pub fn new(key: &'a [u8]) -> Self {
        Self {
            storage_key: key,
            item_type: PhantomData,
            serialization_type: PhantomData,
        }
    }
}

impl<'a, T: Serialize + DeserializeOwned, Ser: Serde> KeyedStorage<T, Ser> for ExplicitStorage<'a, T, Ser> {
    fn get_key(&self) -> &[u8] {
        self.storage_key
    }
}

pub trait KeyedStorage<T: Serialize + DeserializeOwned, Ser: Serde> {
    fn get_key(&self) -> &[u8];

    /// Returns StdResult<T> from retrieving the item with the specified key.  Returns a
    /// StdError::NotFound if there is no item with that key
    ///
    /// # Arguments
    ///
    /// * `storage` - a reference to the storage this item is in
    fn load<S: ReadonlyStorage>(&self, storage: &S) -> StdResult<T> {
        Ser::deserialize(
            &storage
                .get(self.get_key())
                .ok_or_else(|| StdError::not_found(type_name::<T>()))?,
        )
    }

    /// Returns StdResult<Option<T>> from retrieving the item with the specified key.  Returns a
    /// None if there is no item with that key
    ///
    /// # Arguments
    ///
    /// * `storage` - a reference to the storage this item is in
    fn may_load<S: ReadonlyStorage>(&self, storage: &S) -> StdResult<Option<T>> {
        match storage.get(self.get_key()) {
            Some(value) => Ser::deserialize(&value).map(Some),
            None => Ok(None),
        }
    }

    /// Returns StdResult<()> resulting from saving an item to storage
    ///
    /// # Arguments
    ///
    /// * `storage` - a mutable reference to the storage this item should go to
    /// * `value` - a reference to the item to store
    fn save<S: Storage>(&self, storage: &mut S, value: &T) -> StdResult<()> {
        storage.set(self.get_key(), &Ser::serialize(value)?);
        Ok(())
    }

    /// Removes an item from storage
    ///
    /// # Arguments
    ///
    /// * `storage` - a mutable reference to the storage this item is in
    fn remove<S: Storage>(&self, storage: &mut S) {
        storage.remove(self.get_key());
    }
}