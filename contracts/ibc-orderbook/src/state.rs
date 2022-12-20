use cosmwasm_schema::cw_serde;
use cw20::Denom;

use cosmwasm_std::{Addr, Coin, IbcEndpoint, Uint128};
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
pub struct Limit {
    pub liquidty: Token,
    pub liquidity_address: Addr,
    pub liquidity_transfer_channel_id: String,
    pub price_per_token: Token,
    pub ask_transfer_channel_id: String,
}

pub const STATE: Item<State> = Item::new("state");

pub const LIMIT_ID: Item<u64> = Item::new("limit_id");

pub const LIMITS_A: Map<u64, Limit> = Map::new("limits_a");

pub const LIMITS_B: Map<u64, Limit> = Map::new("limits_b");
