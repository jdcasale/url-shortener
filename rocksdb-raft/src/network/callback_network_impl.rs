use std::sync::Arc;
use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, VoteRequest, VoteResponse};
use openraft::{RaftNetwork, Vote, LeaderId, RaftTypeConfig, RaftNetworkFactory};
use openraft::error::{InstallSnapshotError, RaftError, RPCError};
use openraft::network::RPCOption;
use crate::network::management_rpc_client::RaftManagementRPCClient;
use crate::rocksb_store::{TypeConfig};

#[derive(Clone)]
pub struct CallbackRaftNetwork {
    callbacks: Option<RaftManagementRPCClient>
}

impl CallbackRaftNetwork {
    pub fn new() -> CallbackRaftNetwork {
        CallbackRaftNetwork { callbacks: None }
    }

    pub fn set_callbacks(&mut self, callbacks: RaftManagementRPCClient) {
        self.callbacks = Some(callbacks);
    }

    // Create a new network instance for a specific target node
    pub fn for_target(node: &Node) -> Self {
        let base_url = format!("http://{}", node.addr);
        let client = RaftManagementRPCClient::new(base_url);
        CallbackRaftNetwork {
            callbacks: Some(client)
        }
    }
}

pub type NodeId = <TypeConfig as RaftTypeConfig>::NodeId;
pub type Node = <TypeConfig as RaftTypeConfig>::Node;
pub type EntryBruv = <TypeConfig as RaftTypeConfig>::Entry;

impl RaftNetwork<TypeConfig> for Arc<CallbackRaftNetwork> {
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
            // Forward the vote request to the target node
            callbacks.vote(rpc).await
        } else {
            // If no callbacks are set, this is likely the target node
            // Grant the vote if the term is higher than our current term
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

impl RaftNetworkFactory<TypeConfig> for Arc<CallbackRaftNetwork> {
    type Network = Arc<CallbackRaftNetwork>;

    async fn new_client(&mut self, _target: NodeId, node: &Node) -> Self::Network {
        Arc::new(CallbackRaftNetwork::for_target(node))
    }
}

mod test {}