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
