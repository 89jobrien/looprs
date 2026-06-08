use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FsMode {
    Read,
    Update,
    #[default]
    Write,
}

impl FsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Update => "update",
            Self::Write => "write",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "read" => Some(Self::Read),
            "update" => Some(Self::Update),
            "write" => Some(Self::Write),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Read => Self::Update,
            Self::Update => Self::Write,
            Self::Write => Self::Read,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Self::Read => 0,
            Self::Update => 1,
            Self::Write => 2,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Read,
            1 => Self::Update,
            2 => Self::Write,
            _ => Self::Write,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_write() {
        assert_eq!(FsMode::default(), FsMode::Write);
    }

    #[test]
    fn test_as_str() {
        assert_eq!(FsMode::Read.as_str(), "read");
        assert_eq!(FsMode::Update.as_str(), "update");
        assert_eq!(FsMode::Write.as_str(), "write");
    }

    #[test]
    fn test_parse_lowercase() {
        assert_eq!(FsMode::parse("read"), Some(FsMode::Read));
        assert_eq!(FsMode::parse("update"), Some(FsMode::Update));
        assert_eq!(FsMode::parse("write"), Some(FsMode::Write));
    }

    #[test]
    fn test_parse_uppercase() {
        assert_eq!(FsMode::parse("READ"), Some(FsMode::Read));
        assert_eq!(FsMode::parse("UPDATE"), Some(FsMode::Update));
        assert_eq!(FsMode::parse("WRITE"), Some(FsMode::Write));
    }

    #[test]
    fn test_parse_mixedcase() {
        assert_eq!(FsMode::parse("ReAd"), Some(FsMode::Read));
        assert_eq!(FsMode::parse("UpDaTe"), Some(FsMode::Update));
        assert_eq!(FsMode::parse("WrItE"), Some(FsMode::Write));
    }

    #[test]
    fn test_parse_with_whitespace() {
        assert_eq!(FsMode::parse("  read  "), Some(FsMode::Read));
        assert_eq!(FsMode::parse("\tupdate\t"), Some(FsMode::Update));
        assert_eq!(FsMode::parse("\nwrite\n"), Some(FsMode::Write));
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(FsMode::parse("unknown"), None);
        assert_eq!(FsMode::parse(""), None);
        assert_eq!(FsMode::parse("READ2"), None);
        assert_eq!(FsMode::parse("writee"), None);
    }

    #[test]
    fn test_next_cycle() {
        assert_eq!(FsMode::Read.next(), FsMode::Update);
        assert_eq!(FsMode::Update.next(), FsMode::Write);
        assert_eq!(FsMode::Write.next(), FsMode::Read);
    }

    #[test]
    fn test_next_full_cycle() {
        let mut mode = FsMode::Read;
        mode = mode.next();
        assert_eq!(mode, FsMode::Update);
        mode = mode.next();
        assert_eq!(mode, FsMode::Write);
        mode = mode.next();
        assert_eq!(mode, FsMode::Read);
    }

    #[test]
    fn test_to_u8() {
        assert_eq!(FsMode::Read.to_u8(), 0);
        assert_eq!(FsMode::Update.to_u8(), 1);
        assert_eq!(FsMode::Write.to_u8(), 2);
    }

    #[test]
    fn test_from_u8() {
        assert_eq!(FsMode::from_u8(0), FsMode::Read);
        assert_eq!(FsMode::from_u8(1), FsMode::Update);
        assert_eq!(FsMode::from_u8(2), FsMode::Write);
    }

    #[test]
    fn test_from_u8_roundtrip() {
        for variant in &[FsMode::Read, FsMode::Update, FsMode::Write] {
            let u8_val = variant.to_u8();
            let recovered = FsMode::from_u8(u8_val);
            assert_eq!(recovered, *variant);
        }
    }

    #[test]
    fn test_from_u8_unknown_values() {
        assert_eq!(FsMode::from_u8(3), FsMode::Write);
        assert_eq!(FsMode::from_u8(255), FsMode::Write);
        assert_eq!(FsMode::from_u8(100), FsMode::Write);
    }

    #[test]
    fn test_serde_serialize() {
        let read = FsMode::Read;
        let json = serde_json::to_string(&read).unwrap();
        assert_eq!(json, r#""read""#);

        let update = FsMode::Update;
        let json = serde_json::to_string(&update).unwrap();
        assert_eq!(json, r#""update""#);

        let write = FsMode::Write;
        let json = serde_json::to_string(&write).unwrap();
        assert_eq!(json, r#""write""#);
    }

    #[test]
    fn test_serde_deserialize() {
        let read: FsMode = serde_json::from_str(r#""read""#).unwrap();
        assert_eq!(read, FsMode::Read);

        let update: FsMode = serde_json::from_str(r#""update""#).unwrap();
        assert_eq!(update, FsMode::Update);

        let write: FsMode = serde_json::from_str(r#""write""#).unwrap();
        assert_eq!(write, FsMode::Write);
    }

    #[test]
    fn test_serde_roundtrip() {
        for variant in &[FsMode::Read, FsMode::Update, FsMode::Write] {
            let json = serde_json::to_string(variant).unwrap();
            let recovered: FsMode = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered, *variant);
        }
    }
}
