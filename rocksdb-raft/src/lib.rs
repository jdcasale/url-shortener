#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use openraft::{Config, RaftNetwork, Vote};
use openraft::raft::VoteRequest;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task;

use crate::app::App;
use crate::network::api;
use crate::network::management;
use network::no_op_network_impl::NodeId;
use crate::rocksb_store::TypeConfig;
use crate::store::new_storage;
use crate::store::Request;

pub mod app;
pub mod client;
pub mod network;
pub mod store;
pub mod rocksb_store;

pub type SnapshotData = Cursor<Vec<u8>>;

pub mod typ {
    use openraft::error::Infallible;
    use crate::network::no_op_network_impl::{Node, NodeId};
    use crate::rocksb_store::TypeConfig;

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

pub async fn start_example_raft_node<P>(
    node_id: NodeId,
    dir: P,
    http_addr: String,
    rpc_addr: String,
) -> std::io::Result<()>
    where
        P: AsRef<Path>,
{
    // Create a configuration for the raft instance.
    let config = Config {
        heartbeat_interval: 250,
        election_timeout_min: 299,
        ..Default::default()
    };

    let config = Arc::new(config.validate().unwrap());

    let (log_store, state_machine_store) = new_storage(&dir).await;

    let kvs = state_machine_store.data.kvs.clone();

    // Create the network layer that will connect and communicate the raft instances and
    // will be used in conjunction with the store created above.
    let network = Arc::new(network::no_op_network_impl::NoopRaftNetwork {});


    // Create a local raft instance.
    let raft = openraft::Raft::new(node_id, config.clone(), network, log_store, state_machine_store).await.unwrap();
    let resp = raft.vote(VoteRequest::new(Vote::new(1, 1), None)).await.unwrap();
    println!("{resp:?}");
    let app = Arc::new(App {
        id: node_id,
        api_addr: http_addr.clone(),
        rpc_addr: rpc_addr.clone(),
        raft,
        key_values: kvs,
        config,
    });

    let echo_service = Arc::new(network::raft::Raft::new(app.clone()));

    let server = toy_rpc::Server::builder().register(echo_service).build();

    let listener = TcpListener::bind(rpc_addr).await.unwrap();
    let handle = task::spawn(async move {
        server.accept_websocket(listener).await.unwrap();
    });

    // Create an application that will store all the instances created above, this will
    // be later used on the actix-web services.
    let mut app: Server = Server::with_state(app);

    management::rest(&mut app);
    api::rest(&mut app);

    app.listen(http_addr.clone()).await?;
    tracing::info!("App Server listening on: {}", http_addr);
    _ = handle.await;
    Ok(())
}


pub async fn start_raft_node<P>(
    node_id: NodeId,
    dir: P,
    http_addr: String,
    rpc_addr: String,
) -> Arc<App>
where
    P: AsRef<Path>,
{
    // Create a configuration for the raft instance.
    let config = Config {
        heartbeat_interval: 250,
        election_timeout_min: 299,
        ..Default::default()
    };

    let config = Arc::new(config.validate().unwrap());

    let (log_store, state_machine_store) = new_storage(&dir).await;

    let kvs = state_machine_store.data.kvs.clone();

    // Create the network layer that will connect and communicate the raft instances and
    // will be used in conjunction with the store created above.
    let network = Arc::new(network::no_op_network_impl::NoopRaftNetwork {});
    // Create a local raft instance.
    let raft = openraft::Raft::new(node_id, config.clone(), network, log_store, state_machine_store).await.unwrap();

    // Apply the vote to the Raft node
    let mut initial_members = BTreeSet::new();
    initial_members.insert(node_id);
    let bruhh = raft.initialize(initial_members).await;
    let metrics = raft.metrics().borrow().clone();
    println!("{metrics:?}");
    let response1 = raft.change_membership(BTreeSet::from([node_id]), false).await.unwrap();

    Arc::new(App {
        id: node_id,
        api_addr: http_addr.clone(),
        rpc_addr: rpc_addr.clone(),
        raft,
        key_values: kvs,
        config,
    })
}