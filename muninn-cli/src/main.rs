use std::str::FromStr;

use anyhow::Result;
use clap::Parser;
use iroh_net::{endpoint, NodeId};

mod args;
mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = args::Args::parse();
    let mut config = config::Config::get_or_create()?;
    println!("I am {}", config.secret_key.public());
    let create_endpoint = || {
        iroh_net::Endpoint::builder()
            .discovery(Box::new(iroh_net::discovery::dns::DnsDiscovery::n0_dns()))
            .secret_key(config.secret_key.clone())
            .bind()
    };
    match args.subcommand {
        args::Subcommand::ListTasks(list_tasks) => {
            let nodes = list_tasks
                .id
                .into_iter()
                .map(|id| {
                    if let Ok(nodeid) = NodeId::from_str(&id) {
                        Ok((nodeid.to_string(), nodeid))
                    } else if let Some(nodeid) = config.nodes.get(&id) {
                        Ok((id, nodeid.clone()))
                    } else {
                        Err(anyhow::anyhow!("Neither node id nor valid alias: {}", id))
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            let endpoint = create_endpoint().await?;
            for (name, id) in nodes {
                println!("Listing tasks for {}", name);
                let connection = endpoint.connect(id.into(), muninn_proto::ALPN).await?;
                let (mut send, mut recv) = connection.open_bi().await?;
                let request = muninn_proto::Request::ListTasks;
                let request = postcard::to_allocvec(&request)?;
                send.write_all(&request).await?;
                send.finish()?;
                let msg = recv.read_to_end(muninn_proto::MAX_RESPONSE_SIZE).await?;
                let msg = postcard::from_bytes::<muninn_proto::ListTasksResponse>(&msg)?;
                for (pid, name) in msg.tasks {
                    println!("{}: {}", pid, name);
                }
            }
        }
        args::Subcommand::Shutdown(shutdown) => {
            println!("Shutting down {:?}", shutdown.id);
        }
        args::Subcommand::AddNode(add_node) => {
            config.nodes.insert(add_node.name, add_node.addr);
            config.save()?;
        }
    }
    Ok(())
}
