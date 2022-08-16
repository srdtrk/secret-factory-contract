use cosmwasm_std::{
    to_binary, Api, Env, Extern, HandleResponse, HandleResult, HumanAddr, InitResponse, InitResult,
    Querier, QueryResult, ReadonlyStorage, StdError, StdResult, Storage,
};
use secret_toolkit::utils::{HandleCallback, Query};

use crate::factory_msg::{
    FactoryHandleMsg, FactoryOffspringInfo, FactoryQueryMsg, IsKeyValidWrapper,
};
use crate::msg::{HandleMsg, InitMsg, QueryAnswer, QueryMsg};
use crate::state::{State, CONTRACT_ADDR, FACTORY_INFO, IS_ACTIVE, OWNER, PASSWORD, STATE};

////////////////////////////////////// Init ///////////////////////////////////////
/// Returns InitResult
///
/// Initializes the offspring contract state.
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
    FACTORY_INFO.save(&mut deps.storage, &msg.factory)?;
    OWNER.save(&mut deps.storage, &msg.owner)?;
    CONTRACT_ADDR.save(&mut deps.storage, &env.contract.address)?;
    PASSWORD.save(&mut deps.storage, &msg.password)?;
    IS_ACTIVE.save(&mut deps.storage, &true)?;

    let state = State {
        label: msg.label.clone(),
        description: msg.description,
        count: msg.count,
    };
    STATE.save(&mut deps.storage, &state)?;

    // perform register callback to factory
    let offspring = FactoryOffspringInfo {
        label: msg.label,
        password: msg.password,
    };
    let reg_offspring_msg = FactoryHandleMsg::RegisterOffspring {
        owner: msg.owner,
        offspring,
    };
    let cosmos_msg =
        reg_offspring_msg.to_cosmos_msg(msg.factory.code_hash, msg.factory.address, None)?;

    Ok(InitResponse {
        messages: vec![cosmos_msg],
        log: vec![],
    })
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
    match msg {
        HandleMsg::Increment {} => try_increment(deps),
        HandleMsg::Reset { count } => try_reset(deps, env, count),
        HandleMsg::Deactivate {} => try_deactivate(deps, env),
    }
}

/// Returns HandleResult
///
/// deactivates the offspring and lets the factory know.
///
/// # Arguments
///
/// * `deps`  - mutable reference to Extern containing all the contract's external dependencies
/// * `env`   - Env of contract's environment
pub fn try_deactivate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    // let mut state: State = load(&mut deps.storage, CONFIG_KEY)?;
    enforce_active(&deps.storage)?;
    let owner = OWNER.load(&deps.storage)?;
    if env.message.sender != owner {
        return Err(StdError::Unauthorized { backtrace: None });
    }
    IS_ACTIVE.save(&mut deps.storage, &false)?;

    // let factory know
    let factory = FACTORY_INFO.load(&deps.storage)?;
    let deactivate_msg = FactoryHandleMsg::DeactivateOffspring { owner }.to_cosmos_msg(
        factory.code_hash,
        factory.address,
        None,
    )?;

    Ok(HandleResponse {
        messages: vec![deactivate_msg],
        log: vec![],
        data: None,
    })
}

/// Returns HandleResult
///
/// increases the counter. Can be executed by anyone.
///
/// # Arguments
///
/// * `deps` - mutable reference to Extern containing all the contract's external dependencies
pub fn try_increment<S: Storage, A: Api, Q: Querier>(deps: &mut Extern<S, A, Q>) -> HandleResult {
    enforce_active(&deps.storage)?;
    let mut state = STATE.load(&deps.storage)?;
    state.count += 1;
    STATE.save(&mut deps.storage, &state)?;

    Ok(HandleResponse::default())
}

/// Returns HandleResult
///
/// resets the counter to count. Can only be executed by owner.
///
/// # Arguments
///
/// * `deps`  - mutable reference to Extern containing all the contract's external dependencies
/// * `env`   - Env of contract's environment
/// * `count` - The value to reset the counter to.
pub fn try_reset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    count: i32,
) -> HandleResult {
    enforce_active(&deps.storage)?;
    let mut state = STATE.load(&deps.storage)?;
    if env.message.sender != OWNER.load(&deps.storage)? {
        return Err(StdError::Unauthorized { backtrace: None });
    }
    state.count = count;
    STATE.save(&mut deps.storage, &state)?;

    Ok(HandleResponse::default())
}

/////////////////////////////////////// Query /////////////////////////////////////
/// Returns QueryResult
///
/// # Arguments
///
/// * `deps` - reference to Extern containing all the contract's external dependencies
/// * `msg` - QueryMsg passed in with the query call
pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::GetCount {
            address,
            viewing_key,
        } => to_binary(&query_count(deps, &address, viewing_key)?),
    }
}

/// Returns StdResult<CountResponse> displaying the count.
///
/// # Arguments
///
/// * `deps` - reference to Extern containing all the contract's external dependencies
/// * `address` - a reference to the address whose viewing key is being validated.
/// * `viewing_key` - String key used to authenticate the query.
fn query_count<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: &HumanAddr,
    viewing_key: String,
) -> StdResult<QueryAnswer> {
    if OWNER.load(&deps.storage)? == *address {
        enforce_valid_viewing_key(deps, address, viewing_key)?;
        let state: State = STATE.load(&deps.storage)?;
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
/// * `deps` - a reference to Extern containing all the contract's external dependencies.
/// * `state` - a reference to the State of the contract.
/// * `address` - a reference to the address whose viewing key is being validated.
/// * `viewing_key` - String key used to authenticate a query.
fn enforce_valid_viewing_key<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: &HumanAddr,
    viewing_key: String,
) -> StdResult<()> {
    let factory = FACTORY_INFO.load(&deps.storage)?;
    let key_valid_msg = FactoryQueryMsg::IsKeyValid {
        address: address.clone(),
        viewing_key,
    };
    let key_valid_response: IsKeyValidWrapper =
        key_valid_msg.query(&deps.querier, factory.code_hash, factory.address)?;
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
fn enforce_active<S: ReadonlyStorage>(storage: &S) -> StdResult<()> {
    if IS_ACTIVE.load(storage)? {
        Ok(())
    } else {
        return Err(StdError::generic_err("This contract is inactive."));
    }
}
