use crate::state::{Limit, Token};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub packet_lifetime: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    CreateLimit {
        price_per_token: Token,
        liquidity_transfer_channel_id: String,
        ask_transfer_channel_id: String,
    },
    UpdateLimit {
        id: u64,
        price_per_token: Token,
    },
    RemoveLimit {
        id: u64,
    },
    AcceptLimit {
        id: u64,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(Limit)]
    GetLimitOrder { side: String, id: u64 },
}

#[cw_serde]
pub enum PacketMsg {
    CreateLimitB {
        id: u64,
        limit: Limit,
    },
    AcceptLimitA {
        id: u64,
        amount: Uint128,
        sender: String,
    },
}
