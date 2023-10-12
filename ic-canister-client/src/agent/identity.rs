use std::path::Path;

use candid::Principal;
use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::agent::EnvelopeContent;
use ic_agent::identity::{BasicIdentity, Secp256k1Identity};
use ic_agent::{Agent, Identity};

use super::AgentError;

pub enum GenericIdentity {
    Secp256k1Identity(Secp256k1Identity),
    BasicIdentity(BasicIdentity),
}

impl TryFrom<&Path> for GenericIdentity {
    type Error = AgentError;

    fn try_from(path: &Path) -> std::result::Result<Self, Self::Error> {
        Secp256k1Identity::from_pem_file(path)
            .map(GenericIdentity::from)
            .or(BasicIdentity::from_pem_file(path).map(GenericIdentity::from))
            .map_err(|e| AgentError::PemError(path.to_path_buf(), e))
    }
}

impl Identity for GenericIdentity {
    fn sender(&self) -> std::result::Result<Principal, String> {
        match self {
            Self::BasicIdentity(identity) => identity.sender(),
            Self::Secp256k1Identity(identity) => identity.sender(),
        }
    }

    fn sign(&self, blob: &EnvelopeContent) -> std::result::Result<ic_agent::Signature, String> {
        match self {
            Self::BasicIdentity(identity) => identity.sign(blob),
            Self::Secp256k1Identity(identity) => identity.sign(blob),
        }
    }

    // fn public_key(&self) -> Option<Vec<u8>> {
    //     match self {
    //         Self::BasicIdentity(identity) => identity.public_key(),
    //         Self::Secp256k1Identity(identity) => identity.public_key(),
    //     }
    // }
}

impl From<Secp256k1Identity> for GenericIdentity {
    fn from(value: Secp256k1Identity) -> Self {
        Self::Secp256k1Identity(value)
    }
}

impl From<BasicIdentity> for GenericIdentity {
    fn from(value: BasicIdentity) -> Self {
        Self::BasicIdentity(value)
    }
}

/// Initialize an IC Agent
pub async fn init_agent(identity_path: &Path, url: &str) -> super::Result<Agent> {
    let identity = GenericIdentity::try_from(identity_path)?;

    let transport = ReqwestHttpReplicaV2Transport::create(url)?;

    let agent = Agent::builder()
        .with_transport(transport)
        .with_identity(identity)
        .build()?;

    agent.fetch_root_key().await?;

    Ok(agent)
}

#[cfg(test)]
mod test {

    use std::path::Path;

    use super::*;

    #[test]
    fn should_get_identity_from_pem_file() {
        let path = Path::new("./tests/identity/identity.pem");

        assert!(GenericIdentity::try_from(path).is_ok());
        assert!(matches!(
            GenericIdentity::try_from(path).unwrap(),
            GenericIdentity::Secp256k1Identity(_)
        ));
    }

    #[test]
    fn should_get_sender_from_identity() {
        let path = Path::new("./tests/identity/identity.pem");
        let identity = GenericIdentity::try_from(path).unwrap();
        let expected =
            Principal::from_text("zrrb4-gyxmq-nx67d-wmbky-k6xyt-byhmw-tr5ct-vsxu4-nuv2g-6rr65-aae")
                .unwrap();

        let principal = identity.sender().unwrap();

        assert_eq!(expected, principal);
    }

    #[test]
    fn identity_should_sign() {
        let path = Path::new("./tests/identity/identity.pem");
        let identity = GenericIdentity::try_from(path).unwrap();

        let envelop = EnvelopeContent::Query {
            ingress_expiry: 123,
            sender: Principal::anonymous(),
            canister_id: Principal::anonymous(),
            method_name: "some".to_owned(),
            arg: vec![],
        };

        let signature = identity.sign(&envelop).unwrap();

        assert!(signature.signature.is_some());
    }
}
