//! Shared serde helpers.

use std::fmt;

use serde::Deserialize;
use serde::de::{self, Visitor};

/// Serde for a `bitflags` type: serialized as a decimal string (Discord
/// does this for permissions), deserialized from a string or a number.
/// Unknown bits are kept (`from_bits_retain`), so new Discord flags survive
/// a round trip.
macro_rules! bitflags_serde_string {
    ($ty:ty) => {
        impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(&self.bits())
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let bits = $crate::serde_util::u64_from_str_or_int(deserializer, "a bitfield")?;
                Ok(Self::from_bits_retain(bits))
            }
        }
    };
}

/// Serde for a `bitflags` type sent as a JSON number (intents, most
/// `flags` fields). Unknown bits are kept.
macro_rules! bitflags_serde_int {
    ($ty:ty, $bits:ty) => {
        impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serde::Serialize::serialize(&self.bits(), serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let bits = <$bits as serde::Deserialize>::deserialize(deserializer)?;
                Ok(Self::from_bits_retain(bits))
            }
        }
    };
}

/// An integer enum with an `Unknown(repr)` variant, so values Discord adds
/// later never fail deserialization (PLAN.md §5.2). Serialized as a number.
macro_rules! int_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident: $repr:ty {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this version does not know yet.
            Unknown($repr),
        }

        impl From<$repr> for $name {
            fn from(value: $repr) -> Self {
                match value {
                    $( $value => Self::$variant, )*
                    other => Self::Unknown(other),
                }
            }
        }

        impl From<$name> for $repr {
            fn from(value: $name) -> Self {
                match value {
                    $( $name::$variant => $value, )*
                    $name::Unknown(other) => other,
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serde::Serialize::serialize(&<$repr>::from(*self), serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                <$repr as serde::Deserialize>::deserialize(deserializer).map(Self::from)
            }
        }
    };
}

/// A string enum with an `Unknown(Box<str>)` variant (PLAN.md §5.2).
macro_rules! str_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $value:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this version does not know yet.
            Unknown(Box<str>),
        }

        impl $name {
            /// The string Discord uses.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $value, )*
                    Self::Unknown(other) => other,
                }
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $( $value => Self::$variant, )*
                    other => Self::Unknown(other.into()),
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = <std::borrow::Cow<'de, str> as serde::Deserialize>::deserialize(deserializer)?;
                Ok(Self::from(raw.as_ref()))
            }
        }
    };
}

pub(crate) use {bitflags_serde_int, bitflags_serde_string, int_enum, str_enum};

/// Fields Discord documents with type `null` whose *presence* means `true`
/// (e.g. role tags `premium_subscriber`). Use with
/// `#[serde(default, deserialize_with = "present", serialize_with = "null_if_true",
/// skip_serializing_if = "std::ops::Not::not")]`.
pub(crate) fn present<'de, D: de::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    de::IgnoredAny::deserialize(deserializer).map(|_| true)
}

/// Counterpart of [`present`]: writes `null` (only called when `true`).
#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes fields by reference
pub(crate) fn null_if_true<S: serde::Serializer>(
    _: &bool,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_unit()
}

/// Deserializes an optional string that borrows from the input when it has
/// no escape sequences (serde's derive does not borrow `Option<Cow<str>>`).
pub(crate) fn option_cow_str<'de, D: de::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<std::borrow::Cow<'de, str>>, D::Error> {
    use std::borrow::Cow;

    struct V;

    impl<'de> Visitor<'de> for V {
        type Value = Option<Cow<'de, str>>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string or null")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_str(self)
        }

        fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
            Ok(Some(Cow::Borrowed(v)))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(Some(Cow::Owned(v.to_owned())))
        }
    }

    deserializer.deserialize_option(V)
}

/// Deserializes a `u64` that Discord sends as a decimal string (snowflakes,
/// permissions) but that may also arrive as a JSON number.
pub(crate) fn u64_from_str_or_int<'de, D: de::Deserializer<'de>>(
    deserializer: D,
    expecting: &'static str,
) -> Result<u64, D::Error> {
    struct V(&'static str);

    impl Visitor<'_> for V {
        type Value = u64;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<u64, E> {
            u64::try_from(v).map_err(|_| E::invalid_value(de::Unexpected::Signed(v), &self))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
            v.parse()
                .map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
        }
    }

    deserializer.deserialize_any(V(expecting))
}
