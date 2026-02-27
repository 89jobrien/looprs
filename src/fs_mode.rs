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
