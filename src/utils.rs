use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
use ic_agent::identity::BasicIdentity;
use ic_agent::Agent;
use std::env;
use std::path::PathBuf;

fn get_identity_path(account_name: &str) -> PathBuf {
    let home_folder = env::var("HOME").unwrap();
    let mut path = PathBuf::new();
    path.push(home_folder);
    path.push(".config/dfx/identity");
    path.push(account_name);
    path.push("identity.pem");

    path
}

pub fn get_identity(account_name: &str) -> BasicIdentity {
    BasicIdentity::from_pem_file(get_identity_path(account_name)).unwrap()
}

pub async fn get_agent(name: &str, url: &str) -> Agent {
    let identity = get_identity(name);

    let t: ReqwestHttpReplicaV2Transport = ReqwestHttpReplicaV2Transport::create(url).unwrap();
    let agent = Agent::builder()
        .with_transport(t)
        .with_identity(identity)
        .build()
        .expect("should work");

    match agent.fetch_root_key().await {
        Ok(_) => {}
        Err(e) => {
            println!("[get_agent] Error: {:?}", e)
        }
    }

    agent
}
