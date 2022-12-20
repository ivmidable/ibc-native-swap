use cosmwasm_std::{
    entry_point, from_slice, Coin, DepsMut, Env, IbcBasicResponse, IbcChannelCloseMsg,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcChannelOpenResponse, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, StdResult, Uint128,
};
use cw20::Denom;

//use crate::error::Never;

use crate::ibc_helpers::{validate_order_and_version, StdAck};

use crate::error::ContractError;
use crate::msg::PacketMsg;
use crate::state::{Limit, LIMITS_A, LIMITS_B, STATE};

pub const IBC_VERSION: &str = "orderbook-1";

#[entry_point]
/// enforces ordering and versioing constraints
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelOpenMsg,
) -> Result<IbcChannelOpenResponse, ContractError> {
    validate_order_and_version(msg.channel(), msg.counterparty_version())
}

#[entry_point]
/// once it's established, we create the reflect contract
pub fn ibc_channel_connect(
    deps: DepsMut,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<IbcBasicResponse, ContractError> {
    validate_order_and_version(msg.channel(), msg.counterparty_version())?;

    let mut state = STATE.load(deps.storage)?;
    if state.endpoint.is_some() {
        return Err(ContractError::AlreadyConnected {});
    }
    state.endpoint = Some(msg.channel().endpoint.clone());

    if state.counterparty_endpoint.is_some() {
        return Err(ContractError::AlreadyConnected {});
    }

    state.counterparty_endpoint = Some(msg.channel().counterparty_endpoint.clone());

    STATE.save(deps.storage, &state)?;

    Ok(IbcBasicResponse::new()
        .add_attribute("method", "ibc_channel_connect")
        .add_attribute("channel", &msg.channel().endpoint.channel_id)
        .add_attribute("port", &msg.channel().endpoint.port_id))
}

#[entry_point]
pub fn ibc_channel_close(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelCloseMsg,
) -> Result<IbcBasicResponse, ContractError> {
    match msg {
        // Error any TX that would cause the channel to close that is
        // coming from the local chain.
        IbcChannelCloseMsg::CloseInit { channel: _ } => Err(ContractError::CantCloseChannel {}),
        // If we're here, something has gone catastrophically wrong on
        // our counterparty chain. Per the `CloseInit` handler above,
        // this contract will _never_ allow its channel to be
        // closed.
        //
        // Note: erroring here would prevent our side of the channel
        // closing (bad because the channel is, for all intents and
        // purposes, closed) so we must allow the transaction through.
        IbcChannelCloseMsg::CloseConfirm { channel: _ } => Ok(IbcBasicResponse::default()),
        //_ => unreachable!("https://github.com/CosmWasm/cosmwasm/pull/1449"),
    }
}

#[entry_point]
pub fn ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, ContractError> {
    let packet_msg: StdResult<PacketMsg> = from_slice(&msg.packet.data);
    /*if packet_msg.is_err() {
        return Ok(IbcReceiveResponse::new()
            .add_attribute("method", "ibc_packet_receive")
            .add_attribute("error", "invalid packet data")
            .set_ack(StdAck::fail("invalid packet data".to_string())));
    }*/

    match packet_msg.unwrap() {
        PacketMsg::CreateLimitB { id, limit } => create_limit_b(deps, env, id, limit, msg),
        PacketMsg::AcceptLimitA { id, amount, sender } => {
            accept_limit_a(deps, env, id, amount, sender, msg)
        }
    }
}

pub fn create_limit_b(
    deps: DepsMut,
    _env: Env,
    id: u64,
    limit: Limit,
    _msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, ContractError> {
    LIMITS_B.save(deps.storage, id, &limit).unwrap();
    Ok(IbcReceiveResponse::new()
        .add_attribute("method", "ibc_packet_receive")
        .set_ack(StdAck::success(&id)))
}

pub fn accept_limit_a(
    deps: DepsMut,
    env: Env,
    id: u64,
    amount: Uint128,
    sender: String,
    _msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, ContractError> {
    let state = STATE.load(deps.storage)?;
    let mut limit = LIMITS_A.load(deps.storage, id)?;
    match limit.liquidty.denom.clone() {
        Denom::Native(denom) => {
            let coin = Coin {
                denom,
                amount: amount.checked_div(limit.price_per_token.amount).unwrap(),
            };
            let transfer_msg = IbcMsg::Transfer {
                channel_id: limit.liquidity_transfer_channel_id.clone(),
                to_address: sender,
                amount: coin,
                timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
            };

            limit.liquidty.amount = limit
                .liquidty
                .amount
                .checked_sub(amount.checked_div(limit.price_per_token.amount).unwrap())
                .unwrap();
            LIMITS_A.save(deps.storage, id, &limit).unwrap();

            return Ok(IbcReceiveResponse::new()
                .add_attribute("method", "ibc_packet_receive")
                .add_message(transfer_msg)
                .set_ack(StdAck::success(&id)));
        }
        Denom::Cw20(_) => unimplemented!(),
    };
}

#[entry_point]
pub fn ibc_packet_ack(
    deps: DepsMut,
    env: Env,
    msg: IbcPacketAckMsg,
) -> Result<IbcBasicResponse, ContractError> {
    // we need to parse the ack based on our request
    let original_packet: PacketMsg = from_slice(&msg.original_packet.data)?;

    match original_packet {
        PacketMsg::AcceptLimitA { id, amount, sender } => {
            let mut limit = LIMITS_B.load(deps.storage, id)?;
            let state = STATE.load(deps.storage)?;
            match limit.price_per_token.denom.clone() {
                Denom::Native(denom) => {
                    let coin = Coin {
                        denom,
                        amount: amount,
                    };
                    let transfer_msg = IbcMsg::Transfer {
                        channel_id: limit.ask_transfer_channel_id.clone(),
                        to_address: limit.liquidity_address.to_string(),
                        amount: coin,
                        timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
                    };

                    limit.liquidty.amount = limit
                        .liquidty
                        .amount
                        .checked_sub(amount.checked_div(limit.price_per_token.amount).unwrap())
                        .unwrap();

                    LIMITS_B.save(deps.storage, id, &limit).unwrap();
                    return Ok(IbcBasicResponse::new()
                        .add_attribute("method", "ibc_packet_ack")
                        .add_message(transfer_msg)
                    );
                }
                Denom::Cw20(_) => unimplemented!(),
            };
        }
        PacketMsg::CreateLimitB { id:_, limit:_ } => {
            return Ok(IbcBasicResponse::new().add_attribute("action", "ibc_packet_ack"))
        },
    }
}

#[entry_point]
pub fn ibc_packet_timeout(
    _deps: DepsMut,
    _env: Env,
    _msg: IbcPacketTimeoutMsg,
) -> StdResult<IbcBasicResponse> {
    //TODO: return funds and roll back state.

    Ok(IbcBasicResponse::new().add_attribute("action", "ibc_packet_timeout"))
}
