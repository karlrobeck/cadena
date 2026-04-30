use std::time::Duration;

use libp2p::{PeerId, identity};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let local_key = identity::Keypair::generate_ed25519();

    let local_peer_id = PeerId::from(local_key.public());

    println!("Local node PeerID: {:?}", local_peer_id);

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(|_| libp2p::swarm::dummy::Behaviour)?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    Ok(())
}
