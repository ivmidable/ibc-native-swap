#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use cw20::Denom;
use cw_utils::{must_pay};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, PacketMsg, QueryMsg};
use crate::state::{Limit, State, Token, LIMITS_A, LIMITS_B, LIMIT_ID, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:ibc-native-swap";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: info.sender.clone(),
        packet_lifetime: msg.packet_lifetime,
        endpoint: None,
        counterparty_endpoint: None,
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;
    LIMIT_ID.save(deps.storage, &0u64)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateLimit {
            price_per_token,
            liquidity_transfer_channel_id,
            ask_transfer_channel_id,
        } => execute::create_limit(
            deps,
            env,
            info,
            price_per_token,
            liquidity_transfer_channel_id,
            ask_transfer_channel_id,
        ),
        ExecuteMsg::AcceptLimit { id } => execute::accept_limit(deps, env, info, id),
        ExecuteMsg::UpdateLimit {
            id,
            price_per_token,
        } => unimplemented!(),
        ExecuteMsg::RemoveLimit { id } => unimplemented!(),
    }
}

pub mod execute {
    use cosmwasm_std::IbcMsg;

    use super::*;

    pub fn create_limit(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        _price_per_token: Token,
        _liquidity_transfer_channel_id: String,
        _ask_transfer_channel_id: String,
    ) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;

        let limit_id = LIMIT_ID.load(deps.storage)?;

        let limit = Limit {
            liquidty: Token {
                denom: Denom::Native(info.funds[0].denom.clone()),
                amount: info.funds[0].amount,
            },
            liquidity_address: info.sender.clone(),
            price_per_token: _price_per_token,
            liquidity_transfer_channel_id: _liquidity_transfer_channel_id,
            ask_transfer_channel_id: _ask_transfer_channel_id,
        };

        LIMITS_A.save(deps.storage, limit_id, &limit)?;

        LIMIT_ID.save(deps.storage, &(limit_id.checked_add(1).unwrap()))?;

        let packet = PacketMsg::CreateLimitB {
            id: limit_id,
            limit: limit.clone(),
        };

        let msg = IbcMsg::SendPacket {
            channel_id: state.endpoint.unwrap().channel_id,
            data: to_binary(&packet)?,
            timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
        };

        Ok(Response::new()
            .add_message(msg)
            .add_attribute("method", "create_limit"))
    }

    pub fn accept_limit(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        id: u64,
    ) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;

        let limit = LIMITS_B.load(deps.storage, id)?;

        match limit.price_per_token.denom {
            Denom::Native(denom) => {
                must_pay(&info, &denom).unwrap();
            }
            Denom::Cw20(_) => unimplemented!(),
        };

        //make sure they send in atleast enough to buy one token on the other side
        if limit.price_per_token.amount > info.clone().funds[0].amount {
            return Err(ContractError::InsufficientFunds {});
        }

        let channel_id = state.endpoint.unwrap().channel_id;

        let accept_msg = PacketMsg::AcceptLimitA {
            id,
            sender: info.sender.to_string(),
            amount: info.funds[0].amount,
        };

        let packet_msg = IbcMsg::SendPacket {
            channel_id: channel_id,
            data: to_binary(&accept_msg)?,
            timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
        };

        Ok(Response::new()
            .add_message(packet_msg)
            .add_attribute("method", "accept_limit"))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetLimitOrder { side, id } => to_binary(&query::get_limit_order(deps, side, id)?),
    }
}

pub mod query {
    use cosmwasm_std::StdError;

    use super::*;

    pub fn get_limit_order(deps: Deps, side: String, id: u64) -> StdResult<Limit> {
        if side == "A".to_string() {
            return Ok(LIMITS_A.load(deps.storage, id)?);
        } else if side == "B".to_string() {
            return Ok(LIMITS_B.load(deps.storage, id)?);
        } else {
            return Err(StdError::generic_err("Invalid side"));
        }
    }
}
