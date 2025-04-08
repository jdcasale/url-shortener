use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, VoteRequest, VoteResponse};
use openraft::RaftMetrics;
use openraft::error::{InstallSnapshotError, RaftError, RPCError, NetworkError};
use crate::store::types::TypeConfig;
use crate::network::callback_network_impl::{Node, NodeId};
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone)]
pub struct RaftManagementRPCClient {
    base_url: String,
    client: reqwest::Client,
}

impl RaftManagementRPCClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::default()
        }
    }

    async fn send_request_with_retries<T, R, E>(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<&T>,
        max_retries: usize,
    ) -> Result<R, RPCError<NodeId, Node, E>>
    where
        T: serde::Serialize + ?Sized,
        R: serde::de::DeserializeOwned,
        E: std::error::Error + serde::de::DeserializeOwned,
    {
        let mut retries = 0;
        let mut delay = Duration::from_millis(5);
        loop {
            // Attempt the request.
            match self.send_request(method, endpoint, body).await {
                Ok(res) => return Ok(res),
                Err(err) => {
                    if retries >= max_retries {
                        return Err(err);
                    }
                    retries += 1;
                    // Log a warning here if desired.
                    tracing::warn!("send_request to {} failed (attempt {}/{}): {:?}. Retrying in {:?}",
                                    endpoint, retries, max_retries, err, delay);
                    sleep(delay).await;
                    delay *= 2;
                }
            }
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
        self.send_request_with_retries("POST", "raft/append_entries", Some(&req), 10).await
    }

    pub async fn install_snapshot(&self, req: InstallSnapshotRequest<TypeConfig>) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId, InstallSnapshotError>>> {
        self.send_request_with_retries("POST", "raft/install_snapshot", Some(&req), 10).await
    }

    pub async fn vote(&self, req: VoteRequest<NodeId>) -> Result<VoteResponse<NodeId>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request_with_retries("POST", "raft/vote", Some(&req), 10).await
    }

    pub async fn add_learner(&self, node_id: NodeId, api_addr: String, rpc_addr: String) -> Result<(), RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request_with_retries("POST", "cluster/add-learner", Some(&(node_id, api_addr, rpc_addr)), 10).await
    }

    pub async fn change_membership(&self, members: BTreeSet<NodeId>) -> Result<(), RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request_with_retries("POST", "cluster/change-membership", Some(&members), 10).await
    }

    pub async fn init(&self) -> Result<(), RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request_with_retries("POST", "cluster/init", None::<&()>, 10).await
    }

    pub async fn metrics(&self) -> Result<RaftMetrics<NodeId, Node>, RPCError<NodeId, Node, RaftError<NodeId>>> {
        self.send_request_with_retries("GET", "cluster/metrics", None::<&()>, 10).await
    }
}
