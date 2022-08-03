use cosmwasm_std::{HumanAddr};

use secret_toolkit_storage::{Item, Keymap};

use crate::{structs::{CodeInfo, StoreOffspringInfo}};

/// pad handle responses and log attributes to blocks of 256 bytes to prevent leaking info based on
/// response size
pub const BLOCK_SIZE: usize = 256;
/// the default number of offspring listed during queries
pub const DEFAULT_PAGE_SIZE: u32 = 200;

/// whether or not the contract is stopped
pub const IS_STOPPED: Item<bool> = Item::new(b"is_stopped");
/// storage for the admin of the contract
pub const ADMIN: Item<HumanAddr> = Item::new(b"admin");
/// storage for the password of the offspring we just instantiated
pub const PENDING_PASSWORD: Item<[u8; 32]> = Item::new(b"pending");
/// storage for the code_id and code_hash of the current offspring
pub const OFFSPRING_CODE: Item<CodeInfo> = Item::new(b"offspring_version");
/// storage for prng seed
pub const PRNG_SEED: Item<Vec<u8>> = Item::new(b"prng_seed");

/// storage for all active/inactive offspring data. (HumanAddr refers to the address of the contract)
pub const OFFSPRING_STORAGE: Keymap<HumanAddr, StoreOffspringInfo> = Keymap::new(b"offspring_store");
/// storage of all active offspring addresses
pub const ACTIVE_STORE: Keymap<HumanAddr, bool> = Keymap::new(b"active");
/// storage of all inactive offspring addresses
pub const INACTIVE_STORE: Keymap<HumanAddr, bool> = Keymap::new(b"inactive");
/// owner's active offspring storage. Meant to be used with a suffix of the user's address. 
pub const OWNERS_ACTIVE: Keymap<HumanAddr, bool> = Keymap::new(b"owners_active");
/// owner's inactive offspring storage. Meant to be used with a suffix of the user's address. 
pub const OWNERS_INACTIVE: Keymap<HumanAddr, bool> = Keymap::new(b"owners_inactive");