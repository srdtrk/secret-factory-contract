use std::{marker::PhantomData};

use schemars::JsonSchema;
use secret_toolkit_serialization::{Bincode2};
use serde::{Deserialize, Serialize};

use cosmwasm_std::{HumanAddr};

use crate::{msg::ContractInfo, storage::{ExplicitStorage}};

macro_rules! new_explicit_storage {
    ($a:expr) => {
        ExplicitStorage {
            storage_key: $a,
            item_type: PhantomData,
            serialization_type: PhantomData,
        }
    };
}

/// pad handle responses and log attributes to blocks of 256 bytes to prevent leaking info based on
/// response size
pub const BLOCK_SIZE: usize = 256;

/// stores factory code hash and address
pub static FACTORY_INFO: ExplicitStorage<ContractInfo, Bincode2> = new_explicit_storage!(b"factory_info");
/// address of the owner associated to this offspring contract
pub static OWNER: ExplicitStorage<HumanAddr, Bincode2> = new_explicit_storage!(b"owner");
/// address of this offspring contract
pub static CONTRACT_ADDR: ExplicitStorage<HumanAddr, Bincode2> = new_explicit_storage!(b"contract_addr");
/// stores whether or not the contract is still active
pub static IS_ACTIVE: ExplicitStorage<bool, Bincode2> = new_explicit_storage!(b"active");
/// stores the password used to authenticate this contract to the factory
pub static PASSWORD: ExplicitStorage<[u8; 32], Bincode2> = new_explicit_storage!(b"password");
/// used to store the state of this template contract
pub static STATE: ExplicitStorage<State, Bincode2> = new_explicit_storage!(b"state");


/// State of the offspring contract
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// label used when initializing offspring
    pub label: String,
    /// Optional text description of this offspring
    pub description: Option<String>,
    
    /// the count for the counter
    pub count: i32,
}