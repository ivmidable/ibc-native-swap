#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
use cw20::Denom;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, Swap, STATE, SWAPS_A, SWAPS_B, SWAP_ID};

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
    SWAP_ID.save(deps.storage, &0u64)?;

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
        ExecuteMsg::CreateSwap {
            ask,
            deposit_transfer_channel_id,
            ask_transfer_channel_id,
        } => execute::create(
            deps,
            env,
            info,
            ask,
            deposit_transfer_channel_id,
            ask_transfer_channel_id,
        ),
        ExecuteMsg::AcceptSwap { id } => execute::accept(deps, env, info, id),
    }
}

pub mod execute {
    use cosmwasm_std::IbcMsg;
    use cw_utils::{must_pay, one_coin};

    use crate::{msg::PacketMsg, state::Token};

    use super::*;

    pub fn create(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        ask: Token,
        deposit_transfer_channel_id: String,
        ask_transfer_channel_id: String,
    ) -> Result<Response, ContractError> {
        one_coin(&info).unwrap();

        let state = STATE.load(deps.storage)?;

        let swap_id = SWAP_ID.load(deps.storage)?;
        let swap = Swap {
            deposit: Token {
                denom: Denom::Native(info.funds[0].denom.clone()),
                amount: info.funds[0].amount,
            },
            deposit_address: info.sender.clone(),
            deposit_transfer_channel_id,
            ask: ask,
            ask_address: None,
            ask_transfer_channel_id,
        };

        let packet = PacketMsg::CreateSideB {
            id: swap_id,
            swap: swap.clone(),
        };
        let msg = IbcMsg::SendPacket {
            channel_id: state.endpoint.unwrap().channel_id,
            data: to_binary(&packet)?,
            timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
        };

        SWAPS_A.save(deps.storage, swap_id, &swap)?;
        SWAP_ID.save(deps.storage, &(swap_id.checked_add(1).unwrap()))?;

        Ok(Response::new()
            .add_message(msg)
            .add_attribute("method", "create_swap"))
    }

    pub fn accept(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        id: u64,
    ) -> Result<Response, ContractError> {
        let state = STATE.load(deps.storage)?;

        let swap = SWAPS_B.load(deps.storage, id)?;

        if swap.ask_address.is_some() {
            if info.sender.to_string() != swap.ask_address.unwrap() {
                return Err(ContractError::Unauthorized {});
            }
        }

        match swap.ask.denom {
            Denom::Native(denom) => {
                must_pay(&info, &denom).unwrap();
            }
            Denom::Cw20(_) => unimplemented!(),
        };

        if swap.ask.amount > info.clone().funds[0].amount {
            return Err(ContractError::InsufficientFunds {});
        }
        //let mut msgs: Vec<IbcMsg> = vec![];

        let channel_id = state.endpoint.unwrap().channel_id;

        /*let transfer_msg = IbcMsg::Transfer {
            channel_id: transfer_channel_id,
            to_address: swap.deposit_address.to_string(),
            amount: coin,
            timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
        };*/

        //msgs.push(transfer_msg);

        let accept_msg = PacketMsg::AcceptSideA {
            id,
            sender: info.sender.to_string(),
        };

        let packet_msg = IbcMsg::SendPacket {
            channel_id: channel_id,
            data: to_binary(&accept_msg)?,
            timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
        };

        //msgs.push(packet_msg);

        Ok(Response::new()
            //.add_message(transfer_msg)
            .add_message(packet_msg)
            .add_attribute("method", "accept_swap"))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetSwap { side, id } => to_binary(&query::get_swap(deps, side, id)?),
    }
}

pub mod query {
    use cosmwasm_std::StdError;

    use super::*;

    pub fn get_swap(deps: Deps, side: String, id: u64) -> StdResult<Swap> {
        if side == "A".to_string() {
            return Ok(SWAPS_A.load(deps.storage, id)?);
        } else if side == "B".to_string() {
            return Ok(SWAPS_B.load(deps.storage, id)?);
        } else {
            return Err(StdError::generic_err("Invalid side"));
        }
    }
}
