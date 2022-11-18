use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply,
    Response, Storage, SubMsg, SubMsgResult,
};

use secret_toolkit::utils::{pad_handle_result, pad_query_result, InitCallback};

use secret_toolkit::storage::Keyset;
use secret_toolkit::viewing_key::{ViewingKey, ViewingKeyStore};

use crate::error::ContractError;
use crate::state::{BLOCK_SIZE, OFFSPRING_INSTANTIATE_REPLY_ID};
use crate::structs::ReplyOffspringInfo;
use crate::{
    msg::{
        ExecuteMsg, FilterTypes, HandleAnswer, InstantiateMsg, QueryAnswer, QueryMsg,
        ResponseStatus,
    },
    state::{
        ACTIVE_STORE, ADMIN, DEFAULT_PAGE_SIZE, INACTIVE_STORE, IS_STOPPED, OFFSPRING_CODE,
        OFFSPRING_STORAGE, OWNERS_ACTIVE, OWNERS_INACTIVE,
    },
    structs::{CodeInfo, ContractInfo, StoreOffspringInfo},
};

use crate::offspring_msg::OffspringInstantiateMsg;

////////////////////////////////////// Init ///////////////////////////////////////
/// Returns Result<Response, ContractError>
///
/// Initializes the offspring contract state.
///
/// # Arguments
///
/// * `deps`  - DepsMut containing all the contract's external dependencies
/// * `_env`  - Env of contract's environment
/// * `info`  - Carries the info of who sent the message and how much native funds were sent
/// * `msg`   - InitMsg passed in with the instantiation message
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    ADMIN.save(deps.storage, &info.sender)?;
    IS_STOPPED.save(deps.storage, &false)?;
    OFFSPRING_CODE.save(deps.storage, &msg.offspring_code_info)?;

    Ok(Response::new())
}

///////////////////////////////////// Execute //////////////////////////////////////
/// Returns Result<Response, ContractError>
///
/// # Arguments
///
/// * `deps` - DepsMut containing all the contract's external dependencies
/// * `env`  - Env of contract's environment
/// * `info` - Carries the info of who sent the message and how much native funds were sent along
/// * `msg`  - HandleMsg passed in with the execute message
#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let response = match msg {
        ExecuteMsg::CreateOffspring {
            label,
            owner,
            count,
            description,
        } => try_create_offspring(deps, env, label, owner, count, description),
        ExecuteMsg::DeactivateOffspring { owner } => try_deactivate_offspring(deps, info, owner),
        ExecuteMsg::CreateViewingKey { entropy } => try_create_key(deps, env, info, entropy),
        ExecuteMsg::SetViewingKey { key, .. } => try_set_key(deps, info, &key),
        ExecuteMsg::NewOffspringContract {
            offspring_code_info,
        } => try_new_contract(deps, info, offspring_code_info),
        ExecuteMsg::SetStatus { stop } => try_set_status(deps, info, stop),
    };
    pad_handle_result(response, BLOCK_SIZE)
}

/// Returns Result<Response, ContractError>
///
/// create a new offspring
///
/// # Arguments
///
/// * `deps`        - DepsMut containing all the contract's external dependencies
/// * `env`         - Env of contract's environment
/// * `password`    - String containing the password to give the offspring
/// * `owner`       - address of the owner associated to this offspring contract
/// * `count`       - the count for the counter template
/// * `description` - optional free-form text string owner may have used to describe the offspring
fn try_create_offspring(
    deps: DepsMut,
    env: Env,
    label: String,
    owner: String,
    count: i32,
    description: Option<String>,
) -> Result<Response, ContractError> {
    if IS_STOPPED.load(deps.storage)? {
        return Err(ContractError::Stopped {});
    }

    let owner_addr = deps.api.addr_validate(&owner)?;

    let factory = ContractInfo {
        code_hash: env.contract.code_hash,
        address: env.contract.address,
    };

    let initmsg = OffspringInstantiateMsg {
        factory,
        label: label.clone(),
        owner: owner_addr,
        count,
        description,
    };

    let offspring_code = OFFSPRING_CODE.load(deps.storage)?;
    let init_submsg = SubMsg::reply_always(
        initmsg.to_cosmos_msg(
            label,
            offspring_code.code_id,
            offspring_code.code_hash,
            None,
        )?,
        OFFSPRING_INSTANTIATE_REPLY_ID,
    );
    // You can instead turn initmsg into a Cosmos message doing the following:
    /*
    let init_cosmos_msg = CosmosMsg::Wasm(WasmMsg::Instantiate {
        code_id: offspring_code.code_id,
        code_hash: offspring_code.code_hash,
        msg: to_binary(&initmsg)?,
        funds: vec![],
        label,
    });
    */

    Ok(Response::new().add_submessage(init_submsg))
}

/// Returns Result<Response, ContractError>
///
/// deactivates the offspring by saving its info and adding/removing it to/from the
/// appropriate lists
///
/// # Arguments
///
/// * `deps`  - DepsMut containing all the contract's external dependencies
/// * `info`  - Carries the info of who sent the message and how much native funds were sent along
/// * `owner` - Addr of offspring's owner
fn try_deactivate_offspring(
    deps: DepsMut,
    info: MessageInfo,
    owner: Addr,
) -> Result<Response, ContractError> {
    let offspring_addr = &info.sender;

    // verify offspring is in active list
    let is_active = ACTIVE_STORE.contains(deps.storage, offspring_addr);
    if !is_active {
        return Err(ContractError::CustomError {
            val: "This offspring is already not active".to_string(),
        });
    }

    // remove from active
    ACTIVE_STORE.remove(deps.storage, offspring_addr)?;

    // save to inactive
    INACTIVE_STORE.insert(deps.storage, offspring_addr)?;

    // remove from owner's active
    OWNERS_ACTIVE
        .add_suffix(owner.to_string().as_bytes())
        .remove(deps.storage, offspring_addr)?;

    // save to owner's inactive
    OWNERS_INACTIVE
        .add_suffix(owner.to_string().as_bytes())
        .insert(deps.storage, offspring_addr)?;

    Ok(Response::new())
}

/// Returns Result<Response, ContractError>
///
/// allows admin to edit the offspring contract version.
///
/// # Arguments
///
/// * `deps`                - DepsMut containing all the contract's external dependencies
/// * `info`                - Carries the info of who sent the message and how much native funds were sent along
/// * `offspring_code_info` - CodeInfo of the new offspring version
fn try_new_contract(
    deps: DepsMut,
    info: MessageInfo,
    offspring_code_info: CodeInfo,
) -> Result<Response, ContractError> {
    // only allow admin to do this
    let sender = info.sender;
    if ADMIN.load(deps.storage)? != sender {
        return Err(ContractError::Unauthorized {});
    }
    OFFSPRING_CODE.save(deps.storage, &offspring_code_info)?;

    let resp_data = to_binary(&HandleAnswer::Status {
        status: ResponseStatus::Success,
        message: None,
    })?;
    Ok(Response::new().set_data(resp_data))
}

/// Returns Result<Response, ContractError>
///
/// allows admin to change the factory status to (dis)allow the creation of new offspring
///
/// # Arguments
///
/// * `deps` - DepsMut containing all the contract's external dependencies
/// * `info` - Carries the info of who sent the message and how much native funds were sent along
/// * `stop` - true if the factory should disallow offspring creation
fn try_set_status(deps: DepsMut, info: MessageInfo, stop: bool) -> Result<Response, ContractError> {
    // only allow admin to do this
    let sender = info.sender;
    if ADMIN.load(deps.storage)? != sender {
        return Err(ContractError::Unauthorized {});
    }
    IS_STOPPED.save(deps.storage, &stop)?;

    let resp_data = to_binary(&HandleAnswer::Status {
        status: ResponseStatus::Success,
        message: None,
    })?;
    Ok(Response::new().set_data(resp_data))
}

/// Returns Result<Response, ContractError>
///
/// create a viewing key
///
/// # Arguments
///
/// * `deps`    - DepsMut containing all the contract's external dependencies
/// * `env`     - Env of contract's environment
/// * `info`    - Carries the info of who sent the message and how much native funds were sent along
/// * `entropy` - string to be used as an entropy source for randomization
fn try_create_key(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    entropy: String,
) -> Result<Response, ContractError> {
    let key = ViewingKey::create(
        deps.storage,
        &info,
        &env,
        info.sender.as_str(),
        entropy.as_bytes(),
    );

    Ok(Response::new().add_attribute("viewing_key", key))
}

/// Returns Result<Response, ContractError>
///
/// sets the viewing key
///
/// # Arguments
///
/// * `deps` - DepsMut containing all the contract's external dependencies
/// * `info` - Carries the info of who sent the message and how much native funds were sent along
/// * `key`  - string slice to be used as the viewing key
fn try_set_key(deps: DepsMut, info: MessageInfo, key: &str) -> Result<Response, ContractError> {
    ViewingKey::set(deps.storage, info.sender.as_str(), key);

    Ok(Response::new().add_attribute("viewing_key", key))
}

/////////////////////////////////////// Reply /////////////////////////////////////
/// Returns Result<Response, ContractError>
///
/// # Arguments
///
/// * `deps` - DepsMut containing all the contract's external dependencies
/// * `msg` - QueryMsg passed in with the query call
#[entry_point]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        OFFSPRING_INSTANTIATE_REPLY_ID => handle_instantiate_reply(deps, msg),
        id => Err(ContractError::UnexpectedReplyId { id }),
    }
}

fn handle_instantiate_reply(deps: DepsMut, msg: Reply) -> Result<Response, ContractError> {
    // The parsing process below can be handled easier if one imports cw-plus
    // See: https://github.com/CosmWasm/cw-plus/blob/main/packages/utils/src/parse_reply.rs
    match msg.result {
        SubMsgResult::Ok(s) => match s.data {
            Some(bin) => {
                let reply_info: ReplyOffspringInfo = from_binary(&bin)?;
                register_offspring_impl(deps, reply_info)
            }
            None => Err(ContractError::CustomError {
                val: "Init didn't response with contract address".to_string(),
            }),
        },
        SubMsgResult::Err(e) => Err(ContractError::CustomError { val: e }),
    }
}

/// Returns Result<Response, ContractError>
///
/// Registers the calling offspring by saving its info and adding it to the appropriate lists
///
/// # Arguments
///
/// * `deps`       - DepsMut containing all the contract's external dependencies
/// * `reply_info` - reference to ReplyOffspringInfo of the offspring that is trying to register
fn register_offspring_impl(
    deps: DepsMut,
    reply_info: ReplyOffspringInfo,
) -> Result<Response, ContractError> {
    // convert register offspring info to storage format
    let offspring = reply_info.to_store_offspring_info();

    // save the offspring info
    OFFSPRING_STORAGE.insert(deps.storage, &reply_info.address, &offspring)?;

    // add active list
    ACTIVE_STORE.insert(deps.storage, &reply_info.address)?;
    // add to owner's active list
    OWNERS_ACTIVE
        .add_suffix(reply_info.owner.to_string().as_bytes())
        .insert(deps.storage, &reply_info.address)?;

    Ok(Response::new().add_attribute("offspring_address", &reply_info.address))
}

/////////////////////////////////////// Query /////////////////////////////////////
/// Returns Result<Binary, ContractError>
///
/// # Arguments
///
/// * `deps` - Deps containing all the contract's external dependencies
/// * `_env` - Env of contract's environment
/// * `msg`  - QueryMsg passed in with the query call
#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    let response = match msg {
        QueryMsg::ListMyOffspring {
            address,
            viewing_key,
            filter,
            start_page,
            page_size,
        } => try_list_my(deps, address, viewing_key, filter, start_page, page_size),
        QueryMsg::ListActiveOffspring {
            start_page,
            page_size,
        } => try_list_active(deps, start_page, page_size),
        QueryMsg::ListInactiveOffspring {
            start_page,
            page_size,
        } => try_list_inactive(deps, start_page, page_size),
        QueryMsg::IsKeyValid {
            address,
            viewing_key,
        } => try_validate_key(deps, &address, viewing_key),
    };
    pad_query_result(response, BLOCK_SIZE)
}

/// Returns StdResult<Binary> indicating whether the address/key pair is valid
///
/// # Arguments
///
/// * `deps`        - Deps containing all the contract's external dependencies
/// * `address`     - a reference to the address whose key should be validated
/// * `viewing_key` - String key used for authentication
fn try_validate_key(
    deps: Deps,
    address: &str,
    viewing_key: String,
) -> Result<Binary, ContractError> {
    Ok(to_binary(&QueryAnswer::IsKeyValid {
        is_valid: is_key_valid(deps.storage, address, viewing_key),
    })?)
}

/// Returns Result<Binary, ContractError> listing the active offspring
///
/// # Arguments
///
/// * `deps`       - Deps containing all the contract's external dependencies
/// * `start_page` - optional start page for the offsprings returned and listed
/// * `page_size`  - optional number of offspring to return in this page
fn try_list_active(
    deps: Deps,
    start_page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Binary, ContractError> {
    Ok(to_binary(&QueryAnswer::ListActiveOffspring {
        active: display_active_or_inactive_list(
            deps.storage,
            None,
            FilterTypes::Active,
            start_page,
            page_size,
        )?,
    })?)
}

/// Returns bool result of validating an address' viewing key
///
/// # Arguments
///
/// * `storage`     - a reference to the contract's storage
/// * `account`     - a reference to the str whose key should be validated
/// * `viewing_key` - String key used for authentication
fn is_key_valid(storage: &dyn Storage, account: &str, viewing_key: String) -> bool {
    ViewingKey::check(storage, account, &viewing_key).is_ok()
}

/// Returns Result<Binary, ContractError> listing the offspring with the address as its owner
///
/// # Arguments
///
/// * `deps`        - Deps containing all the contract's external dependencies
/// * `address`     - String address whose offspring should be listed
/// * `viewing_key` - String key used to authenticate the query
/// * `filter`      - optional choice of display filters
/// * `start_page`  - optional start page for the offsprings returned and listed
/// * `page_size`   - optional number of offspring to return in this page
fn try_list_my(
    deps: Deps,
    address: String,
    viewing_key: String,
    filter: Option<FilterTypes>,
    start_page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Binary, ContractError> {
    let address = deps.api.addr_validate(&address)?;
    // if key matches
    if !is_key_valid(deps.storage, address.as_str(), viewing_key) {
        return Ok(to_binary(&QueryAnswer::ViewingKeyError {
            error: "Wrong viewing key for this address or viewing key not set".to_string(),
        })?);
    }
    let mut active_list: Option<Vec<StoreOffspringInfo>> = None;
    let mut inactive_list: Option<Vec<StoreOffspringInfo>> = None;
    // if no filter default to ALL
    let types = filter.unwrap_or(FilterTypes::All);

    // list the active offspring
    if types == FilterTypes::Active || types == FilterTypes::All {
        active_list = Some(display_active_or_inactive_list(
            deps.storage,
            Some(address.clone()),
            FilterTypes::Active,
            start_page,
            page_size,
        )?);
    }
    // list the inactive offspring
    if types == FilterTypes::Inactive || types == FilterTypes::All {
        inactive_list = Some(display_active_or_inactive_list(
            deps.storage,
            Some(address),
            FilterTypes::Inactive,
            start_page,
            page_size,
        )?);
    }

    Ok(to_binary(&QueryAnswer::ListMyOffspring {
        active: active_list,
        inactive: inactive_list,
    })?)
}

/// Returns Result<Vec<StoreOffspringInfo>, ContractError>
///
/// provide the appropriate list of active/inactive offspring
///
/// # Arguments
///
/// * `storage`    - a reference to the contract's storage
/// * `owner`      - optional owner only whose offspring are listed. If none, then we list all active/inactive
/// * `filter`     - Specify whether you want active or inactive offspring to be listed
/// * `start_page` - optional start page for the offsprings returned and listed
/// * `page_size`  - optional number of offspring to return in this page
fn display_active_or_inactive_list(
    storage: &dyn Storage,
    owner: Option<Addr>,
    filter: FilterTypes,
    start_page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Vec<StoreOffspringInfo>, ContractError> {
    let start_page = start_page.unwrap_or(0);
    let size = page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    let mut list: Vec<StoreOffspringInfo> = vec![];

    let keyset: &Keyset<Addr>;
    let owners_active_store: Keyset<Addr>;
    let owners_inactive_store: Keyset<Addr>;
    match filter {
        FilterTypes::Active => {
            if let Some(owner_addr) = owner {
                owners_active_store = OWNERS_ACTIVE.add_suffix(owner_addr.to_string().as_bytes());
                keyset = &owners_active_store;
            } else {
                keyset = &ACTIVE_STORE;
            }
        }
        FilterTypes::Inactive => {
            if let Some(owner_addr) = owner {
                owners_inactive_store =
                    OWNERS_INACTIVE.add_suffix(owner_addr.to_string().as_bytes());
                keyset = &owners_inactive_store;
            } else {
                keyset = &INACTIVE_STORE;
            }
        }
        FilterTypes::All => {
            return Err(ContractError::CustomError {
                val: "Please select one of active or inactive offspring to list.".to_string(),
            });
        }
    }

    let mut paginated_keys_iter = keyset
        .iter(storage)?
        .skip((start_page as usize) * (size as usize))
        .take(size as usize);

    loop {
        let may_next_elem = paginated_keys_iter.next();
        if let Some(elem) = may_next_elem {
            let contract_addr = elem?;
            let offspring_info =
                OFFSPRING_STORAGE
                    .get(storage, &contract_addr)
                    .ok_or_else(|| ContractError::CustomError {
                        val: "Error occurred while loading offspring data".to_string(),
                    })?;
            list.push(offspring_info);
        } else {
            break;
        }
    }

    Ok(list)
}

/// Returns Result<Binary, ContractError> listing the inactive offspring
///
/// # Arguments
///
/// * `deps`       - Deps containing all the contract's external dependencies
/// * `start_page` - optional start page for the offsprings returned and listed
/// * `page_size`  - optional number of offspring to display
fn try_list_inactive(
    deps: Deps,
    start_page: Option<u32>,
    page_size: Option<u32>,
) -> Result<Binary, ContractError> {
    Ok(to_binary(&QueryAnswer::ListInactiveOffspring {
        inactive: display_active_or_inactive_list(
            deps.storage,
            None,
            FilterTypes::Inactive,
            start_page,
            page_size,
        )?,
    })?)
}
