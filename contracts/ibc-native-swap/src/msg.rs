use crate::state::{Swap, Token};
use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct InstantiateMsg {
    pub packet_lifetime: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    CreateSwap {
        ask: Token,
        deposit_transfer_channel_id: String,
        ask_transfer_channel_id: String,
    },
    AcceptSwap {
        id: u64,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(Swap)]
    GetSwap { side: String, id: u64 },
}

#[cw_serde]
pub enum PacketMsg {
    CreateSideB { id: u64, swap: Swap },
    AcceptSideA { id: u64, sender: String },
}
