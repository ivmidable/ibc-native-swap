use cosmwasm_std::{
    from_binary, from_slice, to_binary, Binary, IbcAcknowledgement, IbcChannel, IbcEndpoint,
    IbcOrder,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{ibc::IBC_VERSION, ContractError};

/// Tries to remove the source prefix from a given class_id. If the
/// class_id does not begin with the given prefix, returns
/// `None`. Otherwise, returns `Some(unprefixed)`.
pub(crate) fn _try_pop_source_prefix<'a>(
    source: &IbcEndpoint,
    class_id: &'a str,
) -> Option<&'a str> {
    let source_prefix = _get_endpoint_prefix(source);
    // This must not panic in the face of non-ascii, or empty
    // strings. We can not trust classID as it comes from an external
    // IBC connection.
    class_id.strip_prefix(&source_prefix)
}

/// Gets the classID prefix for a given IBC endpoint.
pub(crate) fn _get_endpoint_prefix(source: &IbcEndpoint) -> String {
    format!("{}/{}/", source.port_id, source.channel_id)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StdAck {
    Result(Binary),
    Error(String),
}

impl StdAck {
    // create a serialized success message
    pub fn success(data: impl Serialize) -> Binary {
        let res = to_binary(&data).unwrap();
        StdAck::Result(res).ack()
    }

    // create a serialized error message
    pub fn fail(err: String) -> Binary {
        StdAck::Error(err).ack()
    }

    pub fn ack(&self) -> Binary {
        to_binary(self).unwrap()
    }

    pub fn unwrap(self) -> Binary {
        match self {
            StdAck::Result(data) => data,
            StdAck::Error(err) => panic!("{}", err),
        }
    }

    pub fn unwrap_into<T: DeserializeOwned>(self) -> T {
        from_slice(&self.unwrap()).unwrap()
    }

    pub fn unwrap_err(self) -> String {
        match self {
            StdAck::Result(_) => panic!("not an error"),
            StdAck::Error(err) => err,
        }
    }
}

/// Tries to get the error from an ACK. If an error exists, returns
/// Some(error_message). Otherwise, returns `None`.
///
/// NOTE(ekez): there is a special case here where the contents of the
/// ACK we receive are set by the SDK, and not by our counterparty
/// contract. I do not know all cases this will occur, but I do know
/// it happens if a field on the packet data is set to an empty
/// string. That being the case, the SDK will return an error in the
/// form:
///
/// ```json
/// {"error":"Empty attribute value. Key: class_id: invalid event"}
/// ```
///
/// Should this method encounter such an error, it will return a
/// base64 encoded version of the error (as this is what it
/// receives). For example, the above error is returned as:
///
/// ```json
/// "eyJlcnJvciI6IkVtcHR5IGF0dHJpYnV0ZSB2YWx1ZS4gS2V5OiBjbGFzc19pZDogaW52YWxpZCBldmVudCJ9"
/// ```
pub fn try_get_ack_error(ack: &IbcAcknowledgement) -> Option<String> {
    let ack: StdAck =
	// What we can not parse is an ACK fail.
        from_binary(&ack.data).unwrap_or_else(|_| StdAck::Error(ack.data.to_base64()));
    match ack {
        StdAck::Error(e) => Some(e),
        _ => None,
    }
}

/// Validates order and version information for ics721. We expect
/// ics721-1 as the version and an unordered channel.
pub(crate) fn validate_order_and_version(
    channel: &IbcChannel,
    counterparty_version: Option<&str>,
) -> Result<(), ContractError> {
    // We expect an unordered channel here. Ordered channels have the
    // property that if a message is lost the entire channel will stop
    // working until you start it again.
    if channel.order != IbcOrder::Unordered {
        return Err(ContractError::OrderedChannel {});
    }

    if channel.version != IBC_VERSION {
        return Err(ContractError::InvalidVersion {
            actual: channel.version.to_string(),
            expected: IBC_VERSION.to_string(),
        });
    }

    // Make sure that we're talking with a counterparty who speaks the
    // same "protocol" as us.
    //
    // For a connection between chain A and chain B being established
    // by chain A, chain B knows counterparty information during
    // `OpenTry` and chain A knows counterparty information during
    // `OpenAck`. We verify it when we have it but when we don't it's
    // alright.
    if let Some(counterparty_version) = counterparty_version {
        if counterparty_version != IBC_VERSION {
            return Err(ContractError::InvalidVersion {
                actual: counterparty_version.to_string(),
                expected: IBC_VERSION.to_string(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pop_source_simple() {
        assert_eq!(
            _try_pop_source_prefix(
                &IbcEndpoint {
                    port_id: "wasm.address1".to_string(),
                    channel_id: "channel-10".to_string(),
                },
                "wasm.address1/channel-10/address2"
            ),
            Some("address2")
        )
    }

    #[test]
    fn test_pop_source_adversarial() {
        // Empty string.
        assert_eq!(
            _try_pop_source_prefix(
                &IbcEndpoint {
                    port_id: "wasm.address1".to_string(),
                    channel_id: "channel-10".to_string(),
                },
                ""
            ),
            None
        );

        // Non-ASCII
        assert_eq!(
            _try_pop_source_prefix(
                &IbcEndpoint {
                    port_id: "wasm.address1".to_string(),
                    channel_id: "channel-10".to_string(),
                },
                "☯️☯️"
            ),
            None
        );

        // Invalid classID from prohibited '/' characters.
        assert_eq!(
            _try_pop_source_prefix(
                &IbcEndpoint {
                    port_id: "wasm.address1".to_string(),
                    channel_id: "channel-10".to_string(),
                },
                "wasm.addre//1/channel-10/addre//2"
            ),
            None
        );
    }
}
