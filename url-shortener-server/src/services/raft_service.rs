use actix_web::{post, web, HttpResponse, Responder};
use actix_web::web::Json;
use openraft::raft::{AppendEntriesRequest, InstallSnapshotRequest, VoteRequest};
use rocksdb_raft::network;
use rocksdb_raft::store::types::TypeConfig;
use crate::AppStateWithCounter;
use crate::errors::ShortenerErr;

#[post("/raft/append_entries")]
async fn append_entries(req: Json<AppendEntriesRequest<TypeConfig>>,
                        shared_state: web::Data<AppStateWithCounter>) -> impl Responder {
    tracing::debug!("Append Entries {:?}", req);
    shared_state.raft.raft.append_entries(req.0)
        .await
        .map_err(ShortenerErr::Raft)
        .map(|resp| HttpResponse::Ok().json(resp))

}

#[post("/raft/install_snapshot")]
async fn install_snapshot(req: Json<InstallSnapshotRequest<TypeConfig>>,
                          shared_state: web::Data<AppStateWithCounter>) -> impl Responder {
    shared_state.raft.raft.install_snapshot(req.0)
        .await
        .map_err(ShortenerErr::RaftSnapshot)
        .map(|resp| HttpResponse::Ok().json(resp))
}

#[post("/raft/vote")]
async fn vote(req: Json<VoteRequest<u64>>, shared_state: web::Data<AppStateWithCounter>) -> impl Responder {
    shared_state.raft.raft.vote(req.0)
        .await
        .map_err(ShortenerErr::Raft)
        .map(|resp| HttpResponse::Ok().json(resp))
}

#[post("/cluster/add-learner")]
async fn add_learner(
    req: web::Json<(u64, String)>,
    shared_state: web::Data<AppStateWithCounter>
) -> impl Responder {
    let (node_id, rpc_addr) = req.into_inner();
    tracing::info!("Attempting to add learner - node_id: {}, rpc_addr: {}", node_id, rpc_addr);

    let node = network::callback_network_impl::Node {
        addr: rpc_addr.clone(),
    };

    let res = shared_state.raft.raft.add_learner(node_id, node, true).await;
    match res {
        Ok(resp) => {
            tracing::info!("Successfully added learner node {} at {}. Response: {:?}", node_id, rpc_addr, resp);
            HttpResponse::Ok().json(resp)
        }
        Err(e) => {
            tracing::error!("Failed to add learner node {} at {}. Error: {}", node_id, rpc_addr, e);
            HttpResponse::InternalServerError().body(format!("Failed to add learner: {}", e))
        }
    }
}

#[post("/cluster/change-membership")]
async fn change_membership(
    req: web::Json<std::collections::BTreeSet<u64>>,
    shared_state: web::Data<AppStateWithCounter>
) -> impl Responder {
    let node_ids = req.into_inner();
    tracing::info!("Attempting to change membership. New membership: {:?}", node_ids);

    // Get current metrics to log the change
    let current_metrics = shared_state.raft.raft.metrics().borrow().clone();
    tracing::info!("Current cluster state before membership change: {:?}", current_metrics);

    let res = shared_state.raft.raft.change_membership(node_ids.clone(), false).await;
    match res {
        Ok(resp) => {
            tracing::info!("Successfully changed membership to {:?}. Response: {:?}", node_ids, resp);
            HttpResponse::Ok().json(resp)
        }
        Err(e) => {
            tracing::error!("Failed to change membership to {:?}. Error: {}", node_ids, e);
            HttpResponse::InternalServerError().body(format!("Failed to change membership: {}", e))
        }
    }
}
