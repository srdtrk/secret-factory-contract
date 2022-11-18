use cosmwasm_std::Addr;

use secret_toolkit::{
    serialization::Bincode2,
    storage::{Item, Keymap, KeymapBuilder, Keyset, WithoutIter},
};

use crate::structs::{CodeInfo, StoreOffspringInfo};

/// pad handle responses and log attributes to blocks of 256 bytes to prevent leaking info based on
/// response size
pub const BLOCK_SIZE: usize = 256;
/// the default number of offspring listed during queries
pub const DEFAULT_PAGE_SIZE: u32 = 200;
/// This is the id offspring instantiate submessage returns upon reply
pub const OFFSPRING_INSTANTIATE_REPLY_ID: u64 = 1;
/// Revoked permits prefix key
pub const PREFIX_REVOKED_PERMITS: &str = "revoked_permits";

/// whether or not the contract is stopped
pub const IS_STOPPED: Item<bool> = Item::new(b"is_stopped");
/// storage for the admin of the contract
pub const ADMIN: Item<Addr> = Item::new(b"admin");
/// storage for the code_id and code_hash of the current offspring
pub const OFFSPRING_CODE: Item<CodeInfo> = Item::new(b"offspring_version");

/// storage for all active/inactive offspring data. (HumanAddr refers to the address of the contract)
pub static OFFSPRING_STORAGE: Keymap<Addr, StoreOffspringInfo, Bincode2, WithoutIter> =
    KeymapBuilder::new(b"offspring_store")
        .without_iter()
        .build();
/// storage of all active offspring addresses
pub static ACTIVE_STORE: Keyset<Addr> = Keyset::new(b"active");
/// storage of all inactive offspring addresses
pub static INACTIVE_STORE: Keyset<Addr> = Keyset::new(b"inactive");
/// owner's active offspring storage. Meant to be used with a suffix of the user's address.
pub static OWNERS_ACTIVE: Keyset<Addr> = Keyset::new(b"owners_active");
/// owner's inactive offspring storage. Meant to be used with a suffix of the user's address.
pub static OWNERS_INACTIVE: Keyset<Addr> = Keyset::new(b"owners_inactive");
