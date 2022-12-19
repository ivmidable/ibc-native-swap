use cosmwasm_std::{
    entry_point, from_slice, Coin, DepsMut, Env, IbcBasicResponse, IbcChannelCloseMsg,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcChannelOpenResponse, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, StdResult,
};
use cw20::Denom;

//use crate::error::Never;

use crate::ibc_helpers::{validate_order_and_version, StdAck};

use crate::error::ContractError;
use crate::msg::PacketMsg;
use crate::state::{Swap, STATE, SWAPS_A, SWAPS_B};
//use crate::state::PENDING;

pub const IBC_VERSION: &str = "native-escrow-1";

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
        PacketMsg::CreateSideB { id, swap } => create_side_b(deps, env, id, swap, msg),
        PacketMsg::AcceptSideA { id, sender } => accept_side_a(deps, env, id, sender, msg),
    }
}

pub fn create_side_b(
    deps: DepsMut,
    _env: Env,
    id: u64,
    swap: Swap,
    _msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, ContractError> {
    SWAPS_B.save(deps.storage, id, &swap).unwrap();
    Ok(IbcReceiveResponse::new()
        .add_attribute("method", "ibc_packet_receive")
        .set_ack(StdAck::success(&id)))
}

pub fn accept_side_a(
    deps: DepsMut,
    env: Env,
    id: u64,
    sender: String,
    _msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, ContractError> {
    let state = STATE.load(deps.storage)?;
    let swap = SWAPS_A.load(deps.storage, id)?;
    SWAPS_A.remove(deps.storage, id);
    match swap.deposit.denom {
        Denom::Native(denom) => {
            let coin = Coin {
                denom,
                amount: swap.deposit.amount,
            };
            let transfer_msg = IbcMsg::Transfer {
                channel_id: swap.deposit_transfer_channel_id,
                to_address: sender,
                amount: coin,
                timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
            };

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
        PacketMsg::AcceptSideA { id, sender: _ } => {
            let swap = SWAPS_B.load(deps.storage, id)?;
            SWAPS_B.remove(deps.storage, id);
            let state = STATE.load(deps.storage)?;
            match swap.ask.denom {
                Denom::Native(denom) => {
                    let coin = Coin {
                        denom,
                        amount: swap.ask.amount,
                    };
                    let transfer_msg = IbcMsg::Transfer {
                        channel_id: swap.ask_transfer_channel_id,
                        to_address: swap.deposit_address.to_string(),
                        amount: coin,
                        timeout: env.block.time.plus_seconds(state.packet_lifetime).into(),
                    };

                    return Ok(IbcBasicResponse::new()
                        .add_attribute("method", "ibc_packet_ack")
                        .add_message(transfer_msg));
                }
                Denom::Cw20(_) => unimplemented!(),
            };
        }
        PacketMsg::CreateSideB { id: _, swap: _ } => {
            return Ok(IbcBasicResponse::new().add_attribute("action", "ibc_packet_ack"))
        }
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
