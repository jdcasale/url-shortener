use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse};
use crate::rocksb_store::TypeConfig;

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

    pub async fn append_entries(&self, req: AppendEntriesRequest<TypeConfig>) -> Result<AppendEntriesResponse<u64>, reqwest::Error> {
        self.client.post(format!("{}/raft/append_entries", self.base_url))
            .json(&req)
            .send()
            .await?
            .json()
            .await
    }

    // Implement similar methods for install_snapshot, add_learner, change_membership, etc.
}
