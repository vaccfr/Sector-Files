use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FirCode {
    LFBB,
    LFEE,
    LFFF,
    LFMM,
    LFRR,
}

impl FirCode {
    pub const ALL: [FirCode; 5] = [
        FirCode::LFBB,
        FirCode::LFEE,
        FirCode::LFFF,
        FirCode::LFMM,
        FirCode::LFRR,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FirCode::LFBB => "LFBB",
            FirCode::LFEE => "LFEE",
            FirCode::LFFF => "LFFF",
            FirCode::LFMM => "LFMM",
            FirCode::LFRR => "LFRR",
        }
    }
}

impl FromStr for FirCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "LFBB" => Ok(FirCode::LFBB),
            "LFEE" => Ok(FirCode::LFEE),
            "LFFF" => Ok(FirCode::LFFF),
            "LFMM" => Ok(FirCode::LFMM),
            "LFRR" => Ok(FirCode::LFRR),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for FirCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
