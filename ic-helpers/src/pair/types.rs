use ic_cdk::export::candid::{CandidType, Deserialize, Principal};

#[derive(CandidType, Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
pub enum Standard {
    Ledger,
    Erc20,
}

impl Standard {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ledger => "ledger",
            Self::Erc20 => "erc20",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "ledger" => Self::Ledger,
            "erc20" => Self::Erc20,
            _ => panic!("Unknown token standard: {}", s),
        }
    }
}

#[derive(CandidType, Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenInfo {
    pub principal: Principal,
    pub standard: Standard,
}

impl TokenInfo {
    pub fn empty() -> Self {
        Self {
            principal: Principal::anonymous(),
            standard: Standard::Erc20,
        }
    }
}

impl Default for TokenInfo {
    fn default() -> Self {
        Self::empty()
    }
}

/// Configuration of token weights in AMM pool. This is used to configure weights on pool creation.
#[derive(Debug, CandidType, Deserialize)]
pub struct WeightsConfig {
    pub weight0: f64,
    pub weight1: f64,
    pub change_allowed: bool,
}

impl Default for WeightsConfig {
    fn default() -> Self {
        Self {
            weight0: 0.5,
            weight1: 0.5,
            change_allowed: false
        }
    }
}
