use cosmwasm_std::Addr;
use secret_toolkit::utils::InitCallback;
use serde::{Deserialize, Serialize};

use crate::{state::BLOCK_SIZE, structs::ContractInfo};

/// Instantiation message
#[derive(Serialize, Deserialize)]
pub struct OffspringInstantiateMsg {
    /// factory contract code hash and address
    pub factory: ContractInfo,
    /// label used when initializing offspring
    pub label: String,
    /// Optional text description of this offspring
    #[serde(default)]
    pub description: Option<String>,

    pub owner: Addr,
    pub count: i32,
}

impl InitCallback for OffspringInstantiateMsg {
    const BLOCK_SIZE: usize = BLOCK_SIZE;
}
