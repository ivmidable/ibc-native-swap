use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Already Connected")]
    AlreadyConnected {},

    #[error("Not Connected")]
    NotConnected {},

    #[error("only unordered channels are supported")]
    OrderedChannel {},

    #[error("invalid IBC channel version - got ({actual}), expected ({expected})")]
    InvalidVersion { actual: String, expected: String },

    #[error("channels may not be closed")]
    CantCloseChannel {},

    #[error("Insufficient funds")]
    InsufficientFunds {},
}

/// Enum that can never be constructed. Used as an error type where we
/// can not error.
#[derive(Error, Debug)]
pub enum Never {}
