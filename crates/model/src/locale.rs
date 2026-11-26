//! Official Discord interaction/guild locales (`reference.mdx#locales`).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A Discord locale code (e.g. `"en-US"`, `"de"`, `"ja"`), preserving any
/// future Discord locale in [`Locale::Unknown`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Locale {
    /// Indonesian (`id`).
    Id,
    /// Danish (`da`).
    Da,
    /// German (`de`).
    De,
    /// English, UK (`en-GB`).
    EnGb,
    /// English, US (`en-US`).
    EnUs,
    /// Spanish (`es-ES`).
    EsEs,
    /// Spanish, LATAM (`es-419`).
    Es419,
    /// French (`fr`).
    Fr,
    /// Croatian (`hr`).
    Hr,
    /// Italian (`it`).
    It,
    /// Lithuanian (`lt`).
    Lt,
    /// Hungarian (`hu`).
    Hu,
    /// Dutch (`nl`).
    Nl,
    /// Norwegian (`no`).
    No,
    /// Polish (`pl`).
    Pl,
    /// Portuguese, Brazilian (`pt-BR`).
    PtBr,
    /// Romanian (`ro`).
    Ro,
    /// Finnish (`fi`).
    Fi,
    /// Swedish (`sv-SE`).
    SvSe,
    /// Vietnamese (`vi`).
    Vi,
    /// Turkish (`tr`).
    Tr,
    /// Czech (`cs`).
    Cs,
    /// Greek (`el`).
    El,
    /// Bulgarian (`bg`).
    Bg,
    /// Russian (`ru`).
    Ru,
    /// Ukrainian (`uk`).
    Uk,
    /// Hindi (`hi`).
    Hi,
    /// Thai (`th`).
    Th,
    /// Chinese, China (`zh-CN`).
    ZhCn,
    /// Japanese (`ja`).
    Ja,
    /// Chinese, Taiwan (`zh-TW`).
    ZhTw,
    /// Korean (`ko`).
    Ko,
    /// Any other locale code not yet listed.
    Unknown(Box<str>),
}

impl Locale {
    /// Canonical Discord locale string (e.g. `"en-US"`, `"de"`).
    pub fn as_str(&self) -> &str {
        match self {
            Self::Id => "id",
            Self::Da => "da",
            Self::De => "de",
            Self::EnGb => "en-GB",
            Self::EnUs => "en-US",
            Self::EsEs => "es-ES",
            Self::Es419 => "es-419",
            Self::Fr => "fr",
            Self::Hr => "hr",
            Self::It => "it",
            Self::Lt => "lt",
            Self::Hu => "hu",
            Self::Nl => "nl",
            Self::No => "no",
            Self::Pl => "pl",
            Self::PtBr => "pt-BR",
            Self::Ro => "ro",
            Self::Fi => "fi",
            Self::SvSe => "sv-SE",
            Self::Vi => "vi",
            Self::Tr => "tr",
            Self::Cs => "cs",
            Self::El => "el",
            Self::Bg => "bg",
            Self::Ru => "ru",
            Self::Uk => "uk",
            Self::Hi => "hi",
            Self::Th => "th",
            Self::ZhCn => "zh-CN",
            Self::Ja => "ja",
            Self::ZhTw => "zh-TW",
            Self::Ko => "ko",
            Self::Unknown(s) => s,
        }
    }

    /// Parse a Discord locale string (infallible; unknown codes become [`Locale::Unknown`]).
    pub fn from_code(s: &str) -> Self {
        match s {
            "id" => Self::Id,
            "da" => Self::Da,
            "de" => Self::De,
            "en-GB" => Self::EnGb,
            "en-US" => Self::EnUs,
            "es-ES" => Self::EsEs,
            "es-419" => Self::Es419,
            "fr" => Self::Fr,
            "hr" => Self::Hr,
            "it" => Self::It,
            "lt" => Self::Lt,
            "hu" => Self::Hu,
            "nl" => Self::Nl,
            "no" => Self::No,
            "pl" => Self::Pl,
            "pt-BR" => Self::PtBr,
            "ro" => Self::Ro,
            "fi" => Self::Fi,
            "sv-SE" => Self::SvSe,
            "vi" => Self::Vi,
            "tr" => Self::Tr,
            "cs" => Self::Cs,
            "el" => Self::El,
            "bg" => Self::Bg,
            "ru" => Self::Ru,
            "uk" => Self::Uk,
            "hi" => Self::Hi,
            "th" => Self::Th,
            "zh-CN" => Self::ZhCn,
            "ja" => Self::Ja,
            "zh-TW" => Self::ZhTw,
            "ko" => Self::Ko,
            other => Self::Unknown(other.into()),
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Locale {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_code(s))
    }
}

impl AsRef<str> for Locale {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Serialize for Locale {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Locale {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_code(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_known_and_unknown_locales() {
        assert_eq!(Locale::from_code("de"), Locale::De);
        assert_eq!(Locale::De.as_str(), "de");
        assert_eq!(Locale::from_code("en-US"), Locale::EnUs);
        let unknown = Locale::from_code("xx-YY");
        assert_eq!(unknown.as_str(), "xx-YY");
    }
}
