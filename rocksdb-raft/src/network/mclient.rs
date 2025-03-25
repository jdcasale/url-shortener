use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, VoteRequest, VoteResponse};
use openraft::RaftMetrics;
use openraft::error::{InstallSnapshotError, RaftError, RPCError, NetworkError};
use crate::rocksb_store::TypeConfig;
use crate::network::no_op_network_impl::{Node, NodeId};
use std::collections::BTreeSet;

#[derive(Clone)]
pub struct RaftManagementClient {
    base_url: String,
    client: reqwest::Client,
}

impl RaftManagementClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    async fn send_request<T, R, E>(&self, method: &str, endpoint: &str, body: Option<&T>) -> Result<R, RPCError<NodeId, Node, E>>
    where
        T: serde::Serialize + ?Sized,
        R: serde::de::DeserializeOwned,
        E: std::error::Error + serde::de::DeserializeOwned,
    {
        let url = format!("{}/{}", self.base_url, endpoint);
        let request = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            _ => panic!("Unsupported HTTP method: {}", method),
        };

        let response = request
            .json(&body)
            .send()
            .await
            .map_err(|e| RPCError::Network(NetworkError::new(&e)))?;
        let status = response.status();
        if !status.is_success() {
            return Err(RPCError::Network(NetworkError::new(&std::io::Error::new(std::io::ErrorKind::Other, format!("HTTP error: {}", status)))));
        }

        let response_text = response.text().await.map_err(|e| RPCError::Network(NetworkError::new(&e)))?;

        match serde_json::from_str(&response_text) {
            Ok(value) => Ok(value),
            Err(e) => Err(RPCError::Network(NetworkError::new(&e))),
        }
    }

    pub async fn append_entries(&self, req: AppendEntriesRequest<TypeConfig>) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request("POST", "raft/append_entries", Some(&req)).await
    }

    pub async fn install_snapshot(&self, req: InstallSnapshotRequest<TypeConfig>) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId, InstallSnapshotError>>> {
        self.send_request("POST", "raft/install_snapshot", Some(&req)).await
    }

    pub async fn vote(&self, req: VoteRequest<NodeId>) -> Result<VoteResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request("POST", "raft/vote", Some(&req)).await
    }

    pub async fn add_learner(&self, node_id: NodeId, api_addr: String, rpc_addr: String) -> Result<(), RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request("POST", "cluster/add-learner", Some(&(node_id, api_addr, rpc_addr))).await
    }

    pub async fn change_membership(&self, members: BTreeSet<NodeId>) -> Result<(), RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request("POST", "cluster/change-membership", Some(&members)).await
    }

    pub async fn init(&self) -> Result<(), RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request("POST", "cluster/init", None::<&()>).await
    }

    pub async fn metrics(&self) -> Result<RaftMetrics<NodeId, Node>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request("GET", "cluster/metrics", None::<&()>).await
    }
}
