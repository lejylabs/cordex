//! Dispatch events (opcode 0).
//!
//! Typed variants are added per phase as their models exist; every other
//! event arrives as [`Event::Unknown`] with its raw JSON, so a new Discord
//! event never breaks a shard (PLAN.md D9, §8.3).

use serde_json::value::RawValue;

/// A dispatched gateway event.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// `RESUMED`: a resume finished; missed events were replayed before it.
    Resumed,
    /// An event without a typed variant (yet).
    Unknown(UnknownEvent),
}

/// Raw form of an event without a typed variant.
#[derive(Debug, Clone)]
pub struct UnknownEvent {
    /// Event name (`t`).
    pub name: Box<str>,
    /// Event data (`d`) as raw JSON.
    pub data: Box<RawValue>,
}

impl Event {
    /// Builds an event from a dispatch's name and data. Fails only if a
    /// known event has data that does not match its model.
    pub fn from_dispatch(name: &str, data: Option<&RawValue>) -> serde_json::Result<Self> {
        if let Some(event) = Self::typed(name, data) {
            return event;
        }
        let data = match data {
            Some(raw) => raw.to_owned(),
            None => RawValue::from_string("null".to_owned())?,
        };
        Ok(Self::Unknown(UnknownEvent {
            name: name.into(),
            data,
        }))
    }

    /// Typed events; `None` for names without a typed variant.
    fn typed(name: &str, _data: Option<&RawValue>) -> Option<serde_json::Result<Self>> {
        match name {
            "RESUMED" => Some(Ok(Self::Resumed)),
            _ => None,
        }
    }

    /// The gateway name of the event (`t`).
    pub fn name(&self) -> &str {
        match self {
            Self::Resumed => "RESUMED",
            Self::Unknown(event) => &event.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::payload::Envelope;

    #[test]
    fn resumed_and_unknown() {
        let env = Envelope::parse(r#"{"op":0,"s":2,"t":"RESUMED","d":{}}"#).unwrap();
        let event = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap();
        assert!(matches!(event, Event::Resumed));

        let env = Envelope::parse(r#"{"op":0,"s":3,"t":"BRAND_NEW_EVENT","d":{"x":[1]}}"#).unwrap();
        let event = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap();
        assert_eq!(event.name(), "BRAND_NEW_EVENT");
        match event {
            Event::Unknown(raw) => assert_eq!(raw.data.get(), r#"{"x":[1]}"#),
            Event::Resumed => panic!("expected unknown"),
        }
    }

    #[test]
    fn missing_data_becomes_null() {
        let event = Event::from_dispatch("SOMETHING", None).unwrap();
        match event {
            Event::Unknown(raw) => assert_eq!(raw.data.get(), "null"),
            Event::Resumed => panic!("expected unknown"),
        }
    }
}
