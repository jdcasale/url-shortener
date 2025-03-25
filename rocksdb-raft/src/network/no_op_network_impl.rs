use std::sync::Arc;
use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, VoteRequest, VoteResponse};
use openraft::{RaftNetwork, Vote, LeaderId, RaftTypeConfig, RaftNetworkFactory};
use openraft::error::{InstallSnapshotError, RaftError, RPCError};
use openraft::network::RPCOption;
use crate::network::mclient::RaftManagementClient;
use crate::rocksb_store::{TypeConfig};

#[derive(Clone)]
pub struct NoopRaftNetwork {
    callbacks: Option<RaftManagementClient>
}

impl NoopRaftNetwork {
    pub fn new() -> NoopRaftNetwork {
        NoopRaftNetwork { callbacks: None }
    }

    pub fn set_callbacks(&mut self, callbacks: RaftManagementClient) {
        self.callbacks = Some(callbacks);
    }
}

pub type NodeId = <TypeConfig as RaftTypeConfig>::NodeId;
pub type Node = <TypeConfig as RaftTypeConfig>::Node;
pub type EntryBruv = <TypeConfig as RaftTypeConfig>::Entry;

impl RaftNetwork<TypeConfig> for Arc<NoopRaftNetwork> {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        if let Some(callbacks) = &self.callbacks {
            callbacks.append_entries(rpc).await
        } else {
            Ok(AppendEntriesResponse::Success)
        }
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId, InstallSnapshotError>>> {
        if let Some(callbacks) = &self.callbacks {
            callbacks.install_snapshot(rpc).await
        } else {
            Ok(InstallSnapshotResponse {
                vote: Vote {
                    leader_id: LeaderId::new(rpc.vote.leader_id.node_id, rpc.vote.leader_id.term),
                    committed: true
                }
            })
        }
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        if let Some(callbacks) = &self.callbacks {
            callbacks.vote(rpc).await
        } else {
            Ok(VoteResponse {
                vote: Vote {
                    leader_id: LeaderId::new(rpc.vote.leader_id.node_id, rpc.vote.leader_id.term),
                    committed: true
                },
                vote_granted: true,
                last_log_id: None
            })
        }
    }
}

impl RaftNetworkFactory<TypeConfig> for Arc<NoopRaftNetwork> {
    type Network = Arc<NoopRaftNetwork>;

    async fn new_client(&mut self, _target: NodeId, _node: &Node) -> Self::Network {
        Arc::clone(self)
    }
}

mod test {
    
    
    
    
    

    #[tokio::test]
    async fn test_basic_raft_operations() {
        // let node_id = 1u64;
        // let config = Arc::new(Config::default());
        // let (db_app, state_machine_store) = RocksApp::new("rocksdb.db");
        // let raft = Raft::<TypeConfig>::new(
        //     node_id,
        //     config,
        //     Arc::new(NoopRaftNetwork),
        //     db_app,
        //     state_machine_store.await.unwrap()
        // );
        //
        // // Create a log entry
        // let entry = LongUrlEntry::new(
        //     123412,
        //     "value1".to_string(),
        //     1
        // );
        //
        // // Simulate appending an entry to the log
        // raft.await.unwrap().append_entries(AppendEntriesRequest {
        //
        //     // Fill in with appropriate details
        //     vote: Default::default(),
        //     prev_log_id: None,
        //     entries: vec![entry],
        //     leader_commit: None,
        // }).await.unwrap();
        //
        // // Check that the entry was applied to RocksDB
        // // let stored_value = raft.await.unwrap().get_entry("key1").await.unwrap();
        // // assert_eq!(stored_value, Some("value1".to_string()));
    }
}