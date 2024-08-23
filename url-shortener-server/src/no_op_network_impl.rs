use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, VoteRequest, VoteResponse};
use openraft::{RaftNetwork, Vote, LeaderId, RaftTypeConfig};
use async_trait::async_trait;
use openraft::error::{InstallSnapshotError, RaftError, RPCError};
use openraft::network::RPCOption;
use crate::rocksb_store::{TypeConfig};

pub struct NoopRaftNetwork ;

type NodeId = <TypeConfig as RaftTypeConfig>::NodeId;
type Node = <TypeConfig as RaftTypeConfig>::Node;

#[async_trait]
impl RaftNetwork<TypeConfig> for NoopRaftNetwork {

    async fn append_entries(&mut self, _rpc: AppendEntriesRequest<TypeConfig>, _option: RPCOption) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        Ok(AppendEntriesResponse::Success)
    }

    async fn install_snapshot(&mut self, _rpc: InstallSnapshotRequest<TypeConfig>, _option: RPCOption) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId, InstallSnapshotError>>> {
        Ok(InstallSnapshotResponse {
            vote: Vote {
                leader_id: LeaderId::new(1, 1),
                committed: true
            },
        })
    }

    async fn vote(&mut self, _rpc: VoteRequest<u64>, _option: RPCOption) -> Result<VoteResponse<u64>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(VoteResponse {
            vote: Vote {
            leader_id: LeaderId::new(1, 1),
            committed: true
            },
            vote_granted: true,
            last_log_id: None,
        })
    }

    // Implement other necessary methods with no-op behavior
}
