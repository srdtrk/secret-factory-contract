use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Storage,
};
use secret_toolkit::utils::{HandleCallback, Query};

use crate::error::ContractError;
use crate::factory_msg::{
    FactoryExecuteMsg, FactoryOffspringInfo, FactoryQueryMsg, IsKeyValidWrapper,
};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryAnswer, QueryMsg};
use crate::state::{State, CONTRACT_ADDR, FACTORY_INFO, IS_ACTIVE, OWNER, PASSWORD, STATE};

////////////////////////////////////// Init ///////////////////////////////////////
/// Returns InitResult
///
/// Initializes the offspring con&tract state.
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
) -> StdResult<Response> {
    FACTORY_INFO.save(deps.storage, &msg.factory)?;
    let owner_addr = deps.api.addr_validate(&msg.owner)?;
    OWNER.save(deps.storage, &owner_addr)?;
    CONTRACT_ADDR.save(deps.storage, &env.contract.address)?;
    PASSWORD.save(deps.storage, &msg.password)?;
    IS_ACTIVE.save(deps.storage, &true)?;

    let state = State {
        label: msg.label.clone(),
        description: msg.description,
        count: msg.count,
    };
    STATE.save(deps.storage, &state)?;

    // perform register callback to factory
    let offspring = FactoryOffspringInfo {
        label: msg.label,
        password: msg.password,
    };
    let reg_offspring_msg = FactoryExecuteMsg::RegisterOffspring {
        owner: owner_addr,
        offspring,
    };
    let cosmos_msg = reg_offspring_msg.to_cosmos_msg(
        msg.factory.code_hash,
        msg.factory.address.to_string(),
        None,
    )?;

    Ok(Response::new().add_message(cosmos_msg))
}

///////////////////////////////////// Handle //////////////////////////////////////
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
/// * `deps`  - DepsMut containing all the contract's external dependencies
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

/// Returns HandleResult
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
/// Returns QueryResult
///
/// # Arguments
///
/// * `deps` - Deps containing all the contract's external dependencies
/// * `_env` - Env of contract's environment
/// * `msg`  - QueryMsg passed in with the query call
#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {
            address,
            viewing_key,
        } => to_binary(&query_count(deps, address, viewing_key)?),
    }
}

/// Returns StdResult<CountResponse> displaying the count.
///
/// # Arguments
///
/// * `deps`        - Deps containing all the contract's external dependencies
/// * `address`     - a reference to the address whose viewing key is being validated.
/// * `viewing_key` - String key used to authenticate the query.
fn query_count(deps: Deps, address: String, viewing_key: String) -> StdResult<QueryAnswer> {
    let addr = deps.api.addr_validate(&address)?;
    if OWNER.load(deps.storage)? == addr {
        enforce_valid_viewing_key(deps, &addr, viewing_key)?;
        let state: State = STATE.load(deps.storage)?;
        return Ok(QueryAnswer::CountResponse { count: state.count });
    } else {
        return Err(StdError::generic_err(
            // error message chosen as to not leak information.
            "This address does not have permission and/or viewing key is not valid",
        ));
    }
}

/// Returns StdResult<()>
///
/// makes sure that the address and the viewing key match in the factory contract.
///
/// # Arguments
///
/// * `deps` - Deps containing all the contract's external dependencies.
/// * `state` - a reference to the State of the contract.
/// * `address` - a reference to the address whose viewing key is being validated.
/// * `viewing_key` - String key used to authenticate a query.
fn enforce_valid_viewing_key(deps: Deps, address: &Addr, viewing_key: String) -> StdResult<()> {
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
        return Err(StdError::generic_err(
            // error message chosen as to not leak information.
            "This address does not have permission and/or viewing key is not valid",
        ));
    }
}

/// Returns StdResult<()>
///
/// makes sure that the contract state is active
///
/// # Arguments
///
/// * `state` - a reference to the State of the contract.
fn enforce_active(storage: &dyn Storage) -> StdResult<()> {
    if IS_ACTIVE.load(storage)? {
        Ok(())
    } else {
        return Err(StdError::generic_err("This contract is inactive."));
    }
}
