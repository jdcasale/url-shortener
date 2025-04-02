#![feature(exact_size_is_empty)]
#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use openraft::{BasicNode, Config};

use crate::app::App;
use network::callback_network_impl::NodeId;
use crate::network::callback_network_impl::CallbackRaftNetwork;
use store::types::TypeConfig;
use crate::store::storage::new_storage;

pub mod app;
pub mod client;
pub mod network;
pub mod store;

pub type SnapshotData = Cursor<Vec<u8>>;

pub mod typ {
    use openraft::error::Infallible;
    use crate::network::callback_network_impl::{Node, NodeId};
    use crate::store::types::TypeConfig;

    pub type Entry = openraft::Entry<TypeConfig>;

    pub type RaftError<E = Infallible> = openraft::error::RaftError<NodeId, E>;
    pub type RPCError<E = Infallible> = openraft::error::RPCError<NodeId, Node, RaftError<E>>;

    pub type ClientWriteError = openraft::error::ClientWriteError<NodeId, Node>;
    pub type CheckIsLeaderError = openraft::error::CheckIsLeaderError<NodeId, Node>;
    pub type ForwardToLeader = openraft::error::ForwardToLeader<NodeId, Node>;
    pub type InitializeError = openraft::error::InitializeError<NodeId, Node>;

    pub type ClientWriteResponse = openraft::raft::ClientWriteResponse<TypeConfig>;
}

pub type ExampleRaft = openraft::Raft<TypeConfig>;

type Server = tide::Server<Arc<App>>;

pub async fn start_raft_node<P>(
    node_id: NodeId,
    dir: P,
    http_addr: String,
    network: Arc<CallbackRaftNetwork>
) -> Arc<App>
where
    P: AsRef<Path>,
{
    // Create a configuration for the raft instance.
    let config = Config {
        heartbeat_interval: 250,
        election_timeout_min: 500,
        election_timeout_max: 1500,
        max_payload_entries: 100,
        ..Default::default()
    };

    let config = Arc::new(config.validate().unwrap());

    let (log_store, state_machine_store) = new_storage(&dir).await;

    let kvs = state_machine_store.data.kvs.clone();

    // Create a local raft instance.
    let raft = openraft::Raft::new(node_id, config.clone(), network, log_store, state_machine_store).await.unwrap();

    // Apply the vote to the Raft node
    let mut initial_members = BTreeMap::new();
    initial_members.insert(node_id, BasicNode {addr: http_addr.clone()});

    let initialize = raft.initialize(initial_members).await;
    match initialize {
        Ok(_) => {tracing::info!("initialized new db")}
        Err(err) => {
            println!("did not initialize new db: {err:?}");
            tracing::info!("did not initialize new db: {}", err)
        }
    }
    let metrics = raft.metrics().borrow().clone();
    println!("{metrics:?}");

    Arc::new(App {
        id: node_id,
        api_addr: http_addr.clone(),
        raft,
        key_values: kvs,
        config,
    })
}