// In general, data that is stored for user display may be different from the data used
// for internal functions of the smart contract. That is why we have StoreOffspringInfo.

use cosmwasm_std::Addr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Info needed to instantiate an offspring
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CodeInfo {
    /// code id of the stored offspring contract
    pub code_id: u64,
    /// code hash of the stored offspring contract
    pub code_hash: String,
}

/// code hash and address of a contract
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct ContractInfo {
    /// contract's code hash string
    pub code_hash: String,
    /// contract's address
    pub address: Addr,
}

/// this corresponds to RegisterOffspringInfo in factory, it is used to register
/// an offspring in the factory after the callback.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ReplyOffspringInfo {
    /// label used when initializing offspring
    pub label: String,
    pub owner: Addr,
    pub address: Addr,
    pub code_hash: String,
}

impl ReplyOffspringInfo {
    /// takes the register offspring information and creates a store offspring info struct
    pub fn to_store_offspring_info(&self) -> StoreOffspringInfo {
        StoreOffspringInfo {
            contract: ContractInfo {
                code_hash: self.code_hash.clone(),
                address: self.address.clone(),
            },
            label: self.label.clone(),
        }
    }
}

/// active offspring info for storage/display
#[derive(Serialize, Deserialize, Clone, JsonSchema, Debug)]
pub struct StoreOffspringInfo {
    /// offspring address
    pub contract: ContractInfo,
    /// label used when initializing offspring
    pub label: String,
}

impl CodeInfo {
    pub fn to_contract_info(&self, contract_addr: Addr) -> ContractInfo {
        ContractInfo {
            code_hash: self.code_hash.clone(),
            address: contract_addr,
        }
    }
}
