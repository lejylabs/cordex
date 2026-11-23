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
/// later never fail deserialization. Serialized as a number.
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

/// A string enum with an `Unknown(Box<str>)` variant.
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

/// Replacement for `#[serde(flatten)]` on the deserialize side: keep the
/// object as raw JSON text and parse it twice, once as `A` and once as `B`
/// (each ignores the other's keys). `flatten` would buffer the whole
/// object as a tree of allocations first and generate a second copy of the
/// inner type's serde code; two linear scans are cheaper and smaller.
pub(crate) fn split_flatten<'de, D, A, B>(deserializer: D) -> Result<(A, B), D::Error>
where
    D: de::Deserializer<'de>,
    A: de::DeserializeOwned,
    B: de::DeserializeOwned,
{
    let raw = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
    let a = serde_json::from_str(raw.get()).map_err(de::Error::custom)?;
    let b = serde_json::from_str(raw.get()).map_err(de::Error::custom)?;
    Ok((a, b))
}

/// Visitor for "integer, float or string" values (replaces
/// `#[serde(untagged)]`, which buffers every value first).
pub(crate) struct ScalarVisitor<T>(
    pub(crate) &'static str,
    pub(crate) std::marker::PhantomData<T>,
);

/// What a scalar JSON value turned into.
pub(crate) enum Scalar {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Box<str>),
}

impl<T: TryFrom<Scalar, Error = &'static str>> Visitor<'_> for ScalarVisitor<T> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<T, E> {
        T::try_from(Scalar::Bool(v)).map_err(E::custom)
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<T, E> {
        T::try_from(Scalar::Int(v)).map_err(E::custom)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<T, E> {
        // Above i64::MAX only a float can hold it (like the old untagged order).
        let scalar = i64::try_from(v).map_or_else(
            |_| {
                #[allow(clippy::cast_precision_loss, reason = "same as JSON number semantics")]
                Scalar::Float(v as f64)
            },
            Scalar::Int,
        );
        T::try_from(scalar).map_err(E::custom)
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<T, E> {
        T::try_from(Scalar::Float(v)).map_err(E::custom)
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<T, E> {
        T::try_from(Scalar::Str(v.into())).map_err(E::custom)
    }
}

/// Optional snowflake where Discord uses `"0"` for "none" (e.g. the guild
/// key of `authorizing_integration_owners` in bot DMs).
pub(crate) fn id_or_zero<'de, D: de::Deserializer<'de>, T>(
    deserializer: D,
) -> Result<Option<crate::id::Id<T>>, D::Error> {
    let raw: Option<crate::id::Id<T>> =
        Option::<RawId>::deserialize(deserializer)?.and_then(|raw| crate::id::Id::new(raw.0));
    Ok(raw)
}

/// Snowflake value that may be zero.
struct RawId(u64);

impl<'de> Deserialize<'de> for RawId {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64_from_str_or_int(deserializer, "a snowflake").map(RawId)
    }
}

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
