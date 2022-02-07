

use candid::Principal;
use sha2::{Digest, Sha224};

use ledger_canister::{
    account_identifier::{AccountIdentifier, Subaccount},
    tokens::Tokens,
    Memo, SendArgs,
};


pub static SUB_ACCOUNT_ZERO: Subaccount = Subaccount([0; 32]);
static ACCOUNT_DOMAIN_SEPERATOR: &[u8] = b"\x0Aaccount-id";


pub trait FromPrincipal {
    fn from_principal(account: Principal, sub_account: Option<Subaccount>) -> AccountIdentifier;
}

impl FromPrincipal for AccountIdentifier {
    fn from_principal(account: Principal, sub_account: Option<Subaccount>) -> AccountIdentifier {
        let mut hash = Sha224::new();
        hash.update(ACCOUNT_DOMAIN_SEPERATOR);
        hash.update(account.as_slice());

        let sub_account = sub_account.unwrap_or(SUB_ACCOUNT_ZERO);
        hash.update(&sub_account.0[..]);

        AccountIdentifier {
            hash: hash.finalize().into(),
        }
    }

}

pub trait New {
    fn new(to: Principal, amount: u64, fee: u64) -> SendArgs;
}

impl New for SendArgs {
    fn new(to: Principal, amount: u64, fee: u64) -> Self {
        Self {
            memo: Memo(0x57444857),
            amount: (Tokens::from_e8s(amount) - Tokens::from_e8s(fee)).unwrap(),
            fee: Tokens::from_e8s(fee),
            from_subaccount: None,
            to: AccountIdentifier::from_principal(to, None),
            created_at_time: None,
        }
    }
}