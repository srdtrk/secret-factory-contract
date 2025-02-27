use schemars::JsonSchema;
use secret_toolkit::storage::Item;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;

use crate::msg::ContractInfo;

/// pad handle responses and log attributes to blocks of 256 bytes to prevent leaking info based on
/// response size
pub const BLOCK_SIZE: usize = 256;

/// stores factory code hash and address
pub const FACTORY_INFO: Item<ContractInfo> = Item::new(b"factory_info");
/// address of the owner associated to this offspring contract
pub const OWNER: Item<Addr> = Item::new(b"owner");
/// stores whether or not the contract is still active
pub const IS_ACTIVE: Item<bool> = Item::new(b"active");
/// used to store the state of this template contract
pub const STATE: Item<State> = Item::new(b"state");

/// State of the offspring contract
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct State {
    /// label used when initializing offspring
    pub label: String,
    /// Optional text description of this offspring
    pub description: Option<String>,

    /// the count for the counter
    pub count: i32,
}
