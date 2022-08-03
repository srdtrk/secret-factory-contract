use cosmwasm_std::{
    log, to_binary, Api, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    InitResponse, InitResult, Querier, QueryResult, ReadonlyStorage, StdError, StdResult, Storage,
};

use secret_toolkit::{
    utils::{pad_handle_result, pad_query_result, InitCallback},
    
};

use secret_toolkit_storage::Keymap;
use secret_toolkit_viewing_key::{ViewingKey, ViewingKeyStore};

use crate::{rand::sha_256, state::{DEFAULT_PAGE_SIZE, PRNG_SEED, OFFSPRING_CODE, IS_STOPPED, ADMIN, PENDING_PASSWORD, OFFSPRING_STORAGE, ACTIVE_STORE, OWNERS_ACTIVE, INACTIVE_STORE, OWNERS_INACTIVE},
    msg::{InitMsg, HandleMsg, RegisterOffspringInfo, HandleAnswer, ResponseStatus, QueryMsg, FilterTypes, QueryAnswer}, structs::{ContractInfo, CodeInfo, StoreOffspringInfo}
};
use crate::state::{
    BLOCK_SIZE
};

use crate::{
    offspring_msg::OffspringInitMsg,
    rand::Prng,
};

////////////////////////////////////// Init ///////////////////////////////////////
/// Returns InitResult
///
/// Initializes the factory and creates a prng from the entropy String
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `msg` - InitMsg passed in with the instantiation message
pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    let prng_seed: Vec<u8> = sha_256(base64::encode(msg.entropy).as_bytes()).to_vec();
    
    PRNG_SEED.save(&mut deps.storage, &prng_seed)?;
    ADMIN.save(&mut deps.storage, &env.message.sender)?;
    IS_STOPPED.save(&mut deps.storage, &false)?;
    OFFSPRING_CODE.save(&mut deps.storage, &msg.offspring_code_info)?;

    Ok(InitResponse::default())
}

///////////////////////////////////// Handle //////////////////////////////////////
/// Returns HandleResult
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `msg` - HandleMsg passed in with the execute message
pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    let response = match msg {
        HandleMsg::CreateOffspring {
            label,
            entropy,
            owner,
            count,
            description,
        } => try_create_offspring(deps, env, label, entropy, owner, count, description),
        HandleMsg::RegisterOffspring { owner, offspring } => {
            try_register_offspring(deps, env, owner, &offspring)
        }
        HandleMsg::DeactivateOffspring { owner } => {
            try_deactivate_offspring(deps, env, &owner)
        }
        HandleMsg::CreateViewingKey { entropy } => try_create_key(deps, env, entropy),
        HandleMsg::SetViewingKey { key, .. } => try_set_key(deps, env, &key),
        HandleMsg::NewOffspringContract { offspring_code_info } => {
            try_new_contract(deps, env, offspring_code_info)
        }
        HandleMsg::SetStatus { stop } => try_set_status(deps, env, stop),
    };
    pad_handle_result(response, BLOCK_SIZE)
}

/// Returns [u8;32]
///
/// generates new entropy from block data, does not save it to the contract.
///
/// # Arguments
///
/// * `env` - Env of contract's environment
/// * `seed` - (user generated) seed for rng
/// * `entropy` - Entropy seed saved in the contract
pub fn new_entropy(env: &Env, seed: &[u8], entropy: &[u8]) -> [u8; 32] {
    // 16 here represents the lengths in bytes of the block height and time.
    let entropy_len = 16 + env.message.sender.len() + entropy.len();
    let mut rng_entropy = Vec::with_capacity(entropy_len);
    rng_entropy.extend_from_slice(&env.block.height.to_be_bytes());
    rng_entropy.extend_from_slice(&env.block.time.to_be_bytes());
    rng_entropy.extend_from_slice(&env.message.sender.0.as_bytes());
    rng_entropy.extend_from_slice(entropy);

    let mut rng = Prng::new(seed, &rng_entropy);

    rng.rand_bytes()
}

/// Returns HandleResult
///
/// create a new offspring
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `password` - String containing the password to give the offspring
/// * `owner` - address of the owner associated to this offspring contract
/// * `count` - the count for the counter template
/// * `description` - optional free-form text string owner may have used to describe the offspring
#[allow(clippy::too_many_arguments)]
fn try_create_offspring<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    label: String,
    entropy: String,
    owner: HumanAddr,
    count: i32,
    description: Option<String>,
) -> HandleResult {
    if IS_STOPPED.load(&deps.storage)? {
        return Err(StdError::generic_err(
            "The factory has been stopped. No new offspring can be created",
        ));
    }

    let factory = ContractInfo {
        code_hash: env.clone().contract_code_hash,
        address: env.clone().contract.address,
    };

    // generate and save new prng, and password. (we only register an offspring retuning the matching password)
    let prng_seed: Vec<u8> = PRNG_SEED.load(&deps.storage)?;
    let new_prng_bytes = new_entropy(&env, prng_seed.as_ref(), entropy.as_bytes());
    PRNG_SEED.save(&mut deps.storage, &new_prng_bytes.to_vec())?;

    // store the password for future authentication
    let password = sha_256(&new_prng_bytes);
    PENDING_PASSWORD.save(&mut deps.storage, &password)?;

    let initmsg = OffspringInitMsg {
        factory,
        label: label.clone(),
        password: password.clone(),
        owner,
        count,
        description,
    };

    let offspring_code = OFFSPRING_CODE.load(&deps.storage)?;
    let cosmosmsg = initmsg.to_cosmos_msg(
        label,
        offspring_code.code_id,
        offspring_code.code_hash,
        None,
    )?;

    Ok(HandleResponse {
        messages: vec![cosmosmsg],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Status {
            status: ResponseStatus::Success,
            message: None,
        })?),
    })
}

/// Returns HandleResult
///
/// Registers the calling offspring by saving its info and adding it to the appropriate lists
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `owner` - reference to the address of the offspring's owner
/// * `reg_offspring` - reference to RegisterOffspringInfo of the offspring that is trying to register
fn try_register_offspring<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: HumanAddr,
    reg_offspring: &RegisterOffspringInfo,
) -> HandleResult {
    // verify this is the offspring we are waiting for
    let load_password: Option<[u8; 32]> = PENDING_PASSWORD.may_load(&deps.storage)?;
    let auth_password = load_password
        .ok_or_else(|| StdError::generic_err("Unable to authenticate registration."))?;
    if auth_password != reg_offspring.password {
        return Err(StdError::generic_err(
            "password does not match the offspring we are creating",
        ));
    }
    PENDING_PASSWORD.remove(&mut deps.storage);

    // convert register offspring info to storage format
    let offspring_code_info = OFFSPRING_CODE.load(&deps.storage)?;
    let offspring_info = offspring_code_info.to_contract_info(env.message.sender.clone());
    let offspring = reg_offspring.to_store_offspring_info(offspring_info.clone());

    // save the offspring info
    OFFSPRING_STORAGE.insert(&mut deps.storage, &offspring_info.address, offspring)?;

    // add active list
    ACTIVE_STORE.insert(&mut deps.storage, &offspring_info.address, true)?;
    // add to owner's active list
    OWNERS_ACTIVE.add_suffix(owner.to_string().as_bytes()).insert(&mut deps.storage, &offspring_info.address, true)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("offspring_address", env.message.sender)],
        data: None,
    })
}

/// Returns HandleResult
///
/// deactivates the offspring by saving its info and adding/removing it to/from the
/// appropriate lists
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `owner` - offspring's owner
fn try_deactivate_offspring<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: &HumanAddr,
) -> HandleResult {

    let offspring_addr = &env.message.sender;

    // verify offspring is in active list
    let is_active = ACTIVE_STORE.get(&deps.storage, offspring_addr).unwrap_or(false);
    if !is_active { return Err(StdError::generic_err("This offspring is already not active")); }

    // remove from active
    ACTIVE_STORE.remove(&mut deps.storage, offspring_addr)?;

    // save to inactive
    INACTIVE_STORE.insert(&mut deps.storage, offspring_addr, true)?;
    
    // remove from owner's active
    OWNERS_ACTIVE.add_suffix(owner.to_string().as_bytes()).remove(&mut deps.storage, offspring_addr)?;

    // save to owner's inactive
    OWNERS_INACTIVE.add_suffix(owner.to_string().as_bytes()).insert(&mut deps.storage, offspring_addr, true)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: None,
    })
}

/// Returns HandleResult
///
/// allows admin to edit the offspring contract version.
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `offspring_code_info` - CodeInfo of the new offspring version
fn try_new_contract<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    offspring_code_info: CodeInfo,
) -> HandleResult {
    // only allow admin to do this
    let sender = env.message.sender;
    if ADMIN.load(&deps.storage)? != sender {
        return Err(StdError::generic_err(
            "This is an admin command. Admin commands can only be run from admin address",
        ));
    }
    OFFSPRING_CODE.save(&mut deps.storage, &offspring_code_info)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Status {
            status: ResponseStatus::Success,
            message: None,
        })?),
    })
}

/// Returns HandleResult
///
/// allows admin to change the factory status to (dis)allow the creation of new offspring
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `stop` - true if the factory should disallow offspring creation
fn try_set_status<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    stop: bool,
) -> HandleResult {
    // only allow admin to do this
    let sender = env.message.sender;
    if ADMIN.load(&deps.storage)? != sender {
        return Err(StdError::generic_err(
            "This is an admin command. Admin commands can only be run from admin address",
        ));
    }
    IS_STOPPED.save(&mut deps.storage, &stop)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Status {
            status: ResponseStatus::Success,
            message: None,
        })?),
    })
}

/// Returns HandleResult
///
/// create a viewing key
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `entropy` - string to be used as an entropy source for randomization
fn try_create_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    entropy: String,
) -> HandleResult {
    let key = ViewingKey::create(&mut deps.storage, &env, &env.message.sender, entropy.as_bytes());

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ViewingKey {
            key: format!("{}", key),
        })?),
    })
}

/// Returns HandleResult
///
/// sets the viewing key
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
/// * `env` - Env of contract's environment
/// * `key` - string slice to be used as the viewing key
fn try_set_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    key: &str,
) -> HandleResult {
    ViewingKey::set(&mut deps.storage, &env.message.sender, key);

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ViewingKey {
            key: key.to_string(),
        })?),
    })
}

/////////////////////////////////////// Query /////////////////////////////////////
/// Returns QueryResult
///
/// # Arguments
///
/// * `deps` - reference to Extern containing all the contract's external dependencies
/// * `msg` - QueryMsg passed in with the query call
pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    let response = match msg {
        QueryMsg::ListMyOffspring {
            address,
            viewing_key,
            filter,
            start_page,
            page_size,
        } => try_list_my(deps, address, viewing_key, filter, start_page, page_size),
        QueryMsg::ListActiveOffspring { start_page, page_size } => try_list_active(deps, start_page, page_size),
        QueryMsg::ListInactiveOffspring { start_page, page_size } => try_list_inactive(deps, start_page, page_size),
        QueryMsg::IsKeyValid {
            address,
            viewing_key,
        } => try_validate_key(deps, &address, viewing_key),
    };
    pad_query_result(response, BLOCK_SIZE)
}

/// Returns QueryResult indicating whether the address/key pair is valid
///
/// # Arguments
///
/// * `deps` - reference to Extern containing all the contract's external dependencies
/// * `address` - a reference to the address whose key should be validated
/// * `viewing_key` - String key used for authentication
fn try_validate_key<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: &HumanAddr,
    viewing_key: String,
) -> QueryResult {
    to_binary(&QueryAnswer::IsKeyValid {
        is_valid: is_key_valid(&deps.storage, address, viewing_key),
    })
}

/// Returns QueryResult listing the active offspring
///
/// # Arguments
///
/// * `deps` - reference to Extern containing all the contract's external dependencies
/// * `start_page` - optional start page for the offsprings returned and listed
/// * `page_size` - optional number of offspring to return in this page
fn try_list_active<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    start_page: Option<u32>,
    page_size: Option<u32>,
) -> QueryResult {
    to_binary(&QueryAnswer::ListActiveOffspring {
        active: display_active_or_inactive_list(&deps.storage, None, FilterTypes::Active, start_page, page_size)?,
    })
}

/// Returns bool result of validating an address' viewing key
///
/// # Arguments
///
/// * `storage` - a reference to the contract's storage
/// * `address` - a reference to the address whose key should be validated
/// * `viewing_key` - String key used for authentication
fn is_key_valid<S: ReadonlyStorage>(
    storage: &S,
    address: &HumanAddr,
    viewing_key: String,
) -> bool {
    return ViewingKey::check(storage, address, &viewing_key).is_ok();
}

/// Returns QueryResult listing the offspring with the address as its owner
///
/// # Arguments
///
/// * `deps` - reference to Extern containing all the contract's external dependencies
/// * `address` - a reference to the address whose offspring should be listed
/// * `viewing_key` - String key used to authenticate the query
/// * `filter` - optional choice of display filters
/// * `start_page` - optional start page for the offsprings returned and listed
/// * `page_size` - optional number of offspring to return in this page
fn try_list_my<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    viewing_key: String,
    filter: Option<FilterTypes>,
    start_page: Option<u32>,
    page_size: Option<u32>,
) -> QueryResult {
    // if key matches
    if !is_key_valid(&deps.storage, &address, viewing_key) {
        return to_binary(&QueryAnswer::ViewingKeyError {
            error: "Wrong viewing key for this address or viewing key not set".to_string(),
        });
    }
    let mut active_list: Option<Vec<StoreOffspringInfo>> = None;
    let mut inactive_list: Option<Vec<StoreOffspringInfo>> = None;
    // if no filter default to ALL
    let types = filter.unwrap_or(FilterTypes::All);

    // list the active offspring
    if types == FilterTypes::Active || types == FilterTypes::All {
        active_list = Some( display_active_or_inactive_list(
            &deps.storage,
            Some( address.clone() ),
            FilterTypes::Active,
            start_page,
            page_size,
        )?);
    }
    // list the inactive offspring
    if types == FilterTypes::Inactive || types == FilterTypes::All {
        inactive_list = Some( display_active_or_inactive_list(
            &deps.storage,
            Some( address ),
            FilterTypes::Inactive,
            start_page,
            page_size,
        )?);
    }

    return to_binary(&QueryAnswer::ListMyOffspring {
        active: active_list,
        inactive: inactive_list,
    });
}

/// Returns StdResult<Vec<StoreOffspringInfo>>
///
/// provide the appropriate list of active/inactive offspring
///
/// # Arguments
///
/// * `storage` - a reference to the contract's storage
/// * `owner` - optional owner only whose offspring are listed. If none, then we list all active/inactive
/// * `filter` - Specify whether you want active or inactive offspring to be listed
/// * `start_page` - optional start page for the offsprings returned and listed
/// * `page_size` - optional number of offspring to return in this page
fn display_active_or_inactive_list<S: ReadonlyStorage>(
    storage: &S,
    owner: Option<HumanAddr>,
    filter: FilterTypes,
    start_page: Option<u32>,
    page_size: Option<u32>,
) -> StdResult<Vec<StoreOffspringInfo>> {
    let start_page = start_page.unwrap_or(0);
    let size = page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    let mut list: Vec<StoreOffspringInfo> = vec![];

    let keymap: Keymap<HumanAddr, bool>;
    match filter {
        FilterTypes::Active => {
            if let Some(owner_addr) = owner {
                keymap = OWNERS_ACTIVE.add_suffix(owner_addr.to_string().as_bytes());
            } else {
                keymap = ACTIVE_STORE;
            }
        },
        FilterTypes::Inactive => {
            if let Some(owner_addr) = owner {
                keymap = OWNERS_INACTIVE.add_suffix(owner_addr.to_string().as_bytes());
            } else {
                keymap = INACTIVE_STORE;
            }
        },
        FilterTypes::All => { return Err(StdError::generic_err("Please select one of active or inactive offspring to list.")); },
    }

    let mut paginated_keys_iter = keymap.iter_keys(storage)?.skip((start_page as usize)*(size as usize)).take(size as usize);

    loop {
        let may_next_elem = paginated_keys_iter.next();
        if let Some( elem ) = may_next_elem {
            let contract_addr = elem?;
            let offspring_info = OFFSPRING_STORAGE.get(storage, &contract_addr)
                .ok_or(StdError::generic_err("Error occurred while loading offspring data"))?;
            list.push(offspring_info);
        } else {
            break;
        }
    }
    
    Ok(list)
}

/// Returns QueryResult listing the inactive offspring
///
/// # Arguments
///
/// * `deps` - reference to Extern containing all the contract's external dependencies
/// * `start_page` - optional start page for the offsprings returned and listed
/// * `page_size` - optional number of offspring to display
fn try_list_inactive<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    start_page: Option<u32>,
    page_size: Option<u32>,
) -> QueryResult {
    to_binary(&QueryAnswer::ListInactiveOffspring {
        inactive: display_active_or_inactive_list(&deps.storage, None, FilterTypes::Inactive, start_page, page_size)?,
    })
}
