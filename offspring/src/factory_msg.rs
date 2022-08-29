use cosmwasm_std::Addr;
use serde::{Deserialize, Serialize};

use secret_toolkit::utils::{HandleCallback, Query};

use crate::state::BLOCK_SIZE;

/// Factory handle messages to be used by offspring.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactoryExecuteMsg {
    /// DeactivateOffspring tells the factory that the offspring is inactive.
    DeactivateOffspring {
        /// offspring's owner
        owner: Addr,
    },
}

impl HandleCallback for FactoryExecuteMsg {
    const BLOCK_SIZE: usize = BLOCK_SIZE;
}

/// this corresponds to RegisterOffspringInfo in factory, it is used to register
/// an offspring in the factory after the callback.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FactoryOffspringInfo {
    /// label used when initializing offspring
    pub label: String,
    pub owner: Addr,
    pub address: Addr,
    pub code_hash: String,
}

/// the factory's query messages this offspring will call
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactoryQueryMsg {
    /// authenticates the supplied address/viewing key. This should be called by offspring.
    IsKeyValid {
        /// address whose viewing key is being authenticated
        address: Addr,
        /// viewing key
        viewing_key: String,
    },
}

impl Query for FactoryQueryMsg {
    const BLOCK_SIZE: usize = BLOCK_SIZE;
}

/// result of authenticating address/key pair
#[derive(Serialize, Deserialize, Debug)]
pub struct IsKeyValid {
    pub is_valid: bool,
}

/// IsKeyValid wrapper struct
#[derive(Serialize, Deserialize, Debug)]
pub struct IsKeyValidWrapper {
    pub is_key_valid: IsKeyValid,
}
