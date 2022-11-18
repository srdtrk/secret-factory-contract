use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, Storage,
};
use secret_toolkit::permit::Permit;
use secret_toolkit::utils::{HandleCallback, Query};

use crate::error::ContractError;
use crate::factory_msg::{
    FactoryExecuteMsg, FactoryOffspringInfo, FactoryQueryMsg, IsKeyValidWrapper,
    IsPermitValidWrapper,
};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryAnswer, QueryMsg};
use crate::state::{State, FACTORY_INFO, IS_ACTIVE, OWNER, STATE};

////////////////////////////////////// Init ///////////////////////////////////////
/// Returns Result<Response, ContractError>
///
/// Initializes the offspring contract state.
///
/// # Arguments
///
/// * `deps`  - DepsMut containing all the contract's external dependencies
/// * `env`   - Env of contract's environment
/// * `_info` - Carries the info of who sent the message and how much native funds were sent
/// * `msg`   - InitMsg passed in with the instantiation message
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    FACTORY_INFO.save(deps.storage, &msg.factory)?;
    OWNER.save(deps.storage, &msg.owner)?;
    IS_ACTIVE.save(deps.storage, &true)?;

    let state = State {
        label: msg.label.clone(),
        description: msg.description,
        count: msg.count,
    };
    STATE.save(deps.storage, &state)?;

    // perform register callback to factory
    let offspring_info = FactoryOffspringInfo {
        label: msg.label,
        owner: msg.owner,
        address: env.contract.address,
        code_hash: env.contract.code_hash,
    };

    Ok(Response::new().set_data(to_binary(&offspring_info)?))
}

///////////////////////////////////// Execute //////////////////////////////////////
/// Returns Result<Response, ContractError>
///
/// # Arguments
///
/// * `deps` - DepsMut containing all the contract's external dependencies
/// * `_env` - Env of contract's environment
/// * `info` - Carries the info of who sent the message and how much native funds were sent along
/// * `msg`  - HandleMsg passed in with the execute message
#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Increment {} => try_increment(deps),
        ExecuteMsg::Reset { count } => try_reset(deps, info, count),
        ExecuteMsg::Deactivate {} => try_deactivate(deps, info),
    }
}

/// Returns Result<Response, ContractError>
///
/// deactivates the offspring and lets the factory know.
///
/// # Arguments
///
/// * `deps` - DepsMut containing all the contract's external dependencies
/// * `info` - Carries the info of who sent the message and how much native funds were sent along
pub fn try_deactivate(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    // let mut state: State = load(deps.storage, CONFIG_KEY)?;
    enforce_active(deps.storage)?;
    let owner = OWNER.load(deps.storage)?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }
    IS_ACTIVE.save(deps.storage, &false)?;

    // let factory know
    let factory = FACTORY_INFO.load(deps.storage)?;
    let deactivate_msg = FactoryExecuteMsg::DeactivateOffspring { owner }.to_cosmos_msg(
        factory.code_hash,
        factory.address.to_string(),
        None,
    )?;

    Ok(Response::new().add_message(deactivate_msg))
}

/// Returns Result<Response, ContractError>
///
/// increases the counter. Can be executed by anyone.
///
/// # Arguments
///
/// * `deps` - DepsMut containing all the contract's external dependencies
pub fn try_increment(deps: DepsMut) -> Result<Response, ContractError> {
    enforce_active(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    state.count += 1;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new())
}

/// Returns Result<Response, ContractError>
///
/// resets the counter to count. Can only be executed by owner.
///
/// # Arguments
///
/// * `deps`  - DepsMut containing all the contract's external dependencies
/// * `info`  - Carries the info of who sent the message and how much native funds were sent along
/// * `count` - The value to reset the counter to.
pub fn try_reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
    enforce_active(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    if info.sender != OWNER.load(deps.storage)? {
        return Err(ContractError::Unauthorized {});
    }
    state.count = count;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new())
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
    match msg {
        QueryMsg::GetCount {
            address,
            viewing_key,
            permit,
        } => Ok(to_binary(&query_count(
            deps,
            permit,
            address,
            viewing_key,
        )?)?),
    }
}

/// Returns Result<QueryAnswer, ContractError> displaying the count.
///
/// # Arguments
///
/// * `deps`        - Deps containing all the contract's external dependencies
/// * `permit`      - optional query permit to authenticate the query request. This or viewing key must be provided.
/// * `address`     - optional address whose viewing key is being validated.
/// * `viewing_key` - Optional string key used to authenticate the query.
fn query_count(
    deps: Deps,
    permit: Option<Permit>,
    address: Option<String>,
    viewing_key: Option<String>,
) -> Result<QueryAnswer, ContractError> {
    let addr = if let (Some(address), Some(viewing_key)) = (address, viewing_key) {
        let addr = deps.api.addr_validate(&address)?;
        enforce_valid_viewing_key(deps, &addr, viewing_key)?;
        addr
    } else if let Some(permit) = permit {
        enforce_valid_permit(deps, permit)?
    } else {
        return Err(ContractError::Unauthorized {});
    };

    if OWNER.load(deps.storage)? == addr {
        let state: State = STATE.load(deps.storage)?;
        Ok(QueryAnswer::CountResponse { count: state.count })
    } else {
        Err(ContractError::Unauthorized {})
    }
}

/// Returns Result<(), ContractError>
///
/// makes sure that the address and the viewing key match in the factory contract.
///
/// # Arguments
///
/// * `deps`        - Deps containing all the contract's external dependencies.
/// * `state`       - a reference to the State of the contract.
/// * `address`     - a reference to the address whose viewing key is being validated.
/// * `viewing_key` - String key used to authenticate a query.
fn enforce_valid_viewing_key(
    deps: Deps,
    address: &Addr,
    viewing_key: String,
) -> Result<(), ContractError> {
    let factory = FACTORY_INFO.load(deps.storage)?;
    let key_valid_msg = FactoryQueryMsg::IsKeyValid {
        address: address.clone(),
        viewing_key,
    };
    let key_valid_response: IsKeyValidWrapper =
        key_valid_msg.query(deps.querier, factory.code_hash, factory.address.to_string())?;
    // if authenticated
    if key_valid_response.is_key_valid.is_valid {
        Ok(())
    } else {
        Err(ContractError::ViewingKeyOrUnauthorized {})
    }
}

/// Returns Result<Addr, ContractError>, the address of the permit's signer
///
/// # Arguments
///
/// * `deps`   - Deps containing all the contract's external dependencies
/// * `permit` - permit offered for authentication
fn enforce_valid_permit(deps: Deps, permit: Permit) -> Result<Addr, ContractError> {
    let factory = FACTORY_INFO.load(deps.storage)?;
    let permit_valid_msg = FactoryQueryMsg::IsPermitValid { permit };
    let permit_valid_resp: IsPermitValidWrapper =
        permit_valid_msg.query(deps.querier, factory.code_hash, factory.address.to_string())?;
    if permit_valid_resp.is_key_valid.is_valid {
        permit_valid_resp
            .is_key_valid
            .address
            .ok_or(ContractError::Unauthorized {})
    } else {
        Err(ContractError::Unauthorized {})
    }
}

/// Returns Result<(), ContractError>
///
/// makes sure that the contract state is active
///
/// # Arguments
///
/// * `state` - a reference to the State of the contract.
fn enforce_active(storage: &dyn Storage) -> Result<(), ContractError> {
    if IS_ACTIVE.load(storage)? {
        Ok(())
    } else {
        Err(ContractError::Inactive {})
    }
}
