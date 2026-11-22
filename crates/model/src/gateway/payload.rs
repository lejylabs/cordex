//! Receive-side payloads (events/gateway-events.mdx, "Payload Structure").

use std::borrow::Cow;

use serde::Deserialize;
use serde_json::value::RawValue;

use super::opcode::OpCode;

/// Outer gateway payload `{ op, d, s, t }`, parsed without parsing `d`.
///
/// Borrows from the received (decompressed) text, so parsing the envelope
/// allocates nothing; `d` stays raw JSON until it is needed.
#[derive(Debug, Deserialize)]
pub struct Envelope<'a> {
    /// Opcode.
    pub op: OpCode,
    /// Event data; `None` when missing or `null`.
    #[serde(borrow, default)]
    pub d: Option<&'a RawValue>,
    /// Sequence number; only set for dispatches.
    #[serde(default)]
    pub s: Option<u64>,
    /// Event name; only set for dispatches.
    #[serde(
        borrow,
        default,
        deserialize_with = "crate::serde_util::option_cow_str"
    )]
    pub t: Option<Cow<'a, str>>,
}

impl<'a> Envelope<'a> {
    /// Parses the envelope of one gateway message.
    pub fn parse(json: &'a str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// Parses `d` as `T`; a missing `d` is parsed as `null`.
    pub fn data<T: Deserialize<'a>>(&self) -> serde_json::Result<T> {
        match self.d {
            Some(raw) => serde_json::from_str(raw.get()),
            None => serde_json::from_str("null"),
        }
    }
}

/// Opcode 10 Hello.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[non_exhaustive]
pub struct Hello {
    /// Heartbeat interval in milliseconds.
    pub heartbeat_interval: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_from_the_docs() {
        let json = r#"{"op":10,"d":{"heartbeat_interval":45000}}"#;
        let env = Envelope::parse(json).unwrap();
        assert_eq!(env.op, OpCode::Hello);
        assert_eq!(env.s, None);
        assert_eq!(env.data::<Hello>().unwrap().heartbeat_interval, 45_000);
    }

    #[test]
    fn dispatch_keeps_data_raw_and_borrows_name() {
        let json = r#"{"t":"MESSAGE_CREATE","s":42,"op":0,"d":{"id":"1","nested":[1,2]}}"#;
        let env = Envelope::parse(json).unwrap();
        assert_eq!(env.op, OpCode::Dispatch);
        assert_eq!(env.s, Some(42));
        assert!(matches!(env.t, Some(Cow::Borrowed("MESSAGE_CREATE"))));
        assert_eq!(env.d.unwrap().get(), r#"{"id":"1","nested":[1,2]}"#);
    }

    #[test]
    fn escaped_name_is_copied() {
        // `A\nC` as a JSON escape forces serde_json to unescape into a copy.
        let env = Envelope::parse("{\"op\":0,\"s\":1,\"t\":\"A\\nC\",\"d\":null}").unwrap();
        assert!(matches!(env.t, Some(Cow::Owned(ref s)) if s == "A\nC"));
    }

    #[test]
    fn null_and_bool_data() {
        let env = Envelope::parse(r#"{"op":11,"d":null,"s":null,"t":null}"#).unwrap();
        assert_eq!(env.op, OpCode::HeartbeatAck);
        assert!(env.d.is_none() && env.t.is_none());

        // Opcode 9: `d` says whether the session is resumable.
        let env = Envelope::parse(r#"{"op":9,"d":false}"#).unwrap();
        assert!(!env.data::<bool>().unwrap());
    }

    #[test]
    fn unknown_opcode_and_fields_do_not_fail() {
        let env = Envelope::parse(r#"{"op":77,"d":{},"new_field":true}"#).unwrap();
        assert_eq!(env.op, OpCode::Unknown(77));
    }

    #[test]
    fn invalid_json_is_an_error_not_a_panic() {
        for bad in ["", "{", "[]", r#"{"d":1}"#, r#"{"op":"x"}"#] {
            assert!(Envelope::parse(bad).is_err(), "{bad}");
        }
    }
}
