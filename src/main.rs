use anyhow::anyhow;
use clap::{Parser, Subcommand};
use libp2p::{
    Multiaddr, PeerId,
    futures::StreamExt,
    gossipsub, identify, identity, kad,
    multiaddr::Protocol,
    swarm::{NetworkBehaviour, SwarmEvent},
};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "p2p-node")]
#[command(about = "A simple P2P node for our blockchain", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
}

#[derive(Subcommand)]
enum Commands {
    /// Start in discovery mode (Bootnode)
    Discover,
    /// Connect to the network using a bootnode address
    Node {
        #[arg(short, long)]
        bootnode: Multiaddr,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

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
        .with_behaviour(|key| {
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hasher::write(&mut s, &message.data);
                gossipsub::MessageId::from(std::hash::Hasher::finish(&s).to_string())
            };

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(10))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()?;

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            let kademlia =
                kad::Behaviour::new(local_peer_id, kad::store::MemoryStore::new(local_peer_id));

            let identify =
                identify::Behaviour::new(identify::Config::new("/p2p/1.0.0".into(), key.public()));

            Ok(MyBehaviour {
                gossipsub,
                kademlia,
                identify,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    match cli.command {
        Commands::Discover => {
            println!("Starting as a Bootnode... 🚩");
            // We'll listen on a fixed port later so others can find us!
        }
        Commands::Node { bootnode } => {
            if let Some(Protocol::P2p(peer_id)) = bootnode.iter().last() {
                swarm
                    .behaviour_mut()
                    .kademlia
                    .add_address(&peer_id, bootnode.clone());
                swarm.behaviour_mut().kademlia.bootstrap()?;
                swarm.dial(bootnode)?;
                println!("Dialing bootnode: {:?}", peer_id);
            } else {
                return Err(anyhow!("Bootnode address must include /p2p/<PeerId>"));
            }
        }
    }

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("Local node is listening on {:?}", address)
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            })) => {
                println!("Received Identify from {:?}", peer_id);
                for addr in info.listen_addrs {
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                }
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                message,
                ..
            })) => {
                println!(
                    "Received message on topic {:?}: {:?}",
                    message.topic,
                    String::from_utf8_lossy(&message.data)
                );
            }
            _ => {}
        }
    }
}
