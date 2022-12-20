use cosmwasm_schema::cw_serde;
use cw20::Denom;

use cosmwasm_std::{Addr, IbcEndpoint, Uint128};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct Token {
    pub denom: Denom,
    pub amount: Uint128,
}

#[cw_serde]
pub struct State {
    pub owner: Addr,
    pub packet_lifetime: u64,
    pub endpoint: Option<IbcEndpoint>,
    pub counterparty_endpoint: Option<IbcEndpoint>,
}

#[cw_serde]
pub struct Swap {
    pub deposit: Token,
    pub deposit_address: Addr,
    pub deposit_transfer_channel_id: String,
    pub ask: Token,
    pub ask_address: Option<Addr>,
    pub ask_transfer_channel_id: String,
}

pub const STATE: Item<State> = Item::new("state");

pub const SWAP_ID: Item<u64> = Item::new("swap_id");

pub const SWAPS_A: Map<u64, Swap> = Map::new("swaps_a");

pub const SWAPS_B: Map<u64, Swap> = Map::new("swaps_b");
