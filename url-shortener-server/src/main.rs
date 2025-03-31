extern crate core;

mod errors;
mod params;

use actix_web::{post, web, App, HttpResponse, HttpServer, Responder, get};
use quick_cache::sync::{Cache};
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use actix_web::http::header;
use actix_web::web::Data;
use clap::Parser;
use openraft::BasicNode;
use web::Json;
use rocksdb_raft::{network, start_raft_node};
use openraft::raft::{AppendEntriesRequest, InstallSnapshotRequest, VoteRequest};
use tracing::Level;
use crate::errors::ShortenerErr;
use rocksdb_raft::rocksb_store::{LongUrlEntry, TypeConfig};
use crate::params::Args;
use validator::Validate;
use rocksdb_raft::network::callback_network_impl::{Node, NodeId};

#[derive(Serialize, Deserialize)]
struct Hello {}

struct AppStateWithCounter {
    long_url_lookup: Cache<u64, String>,
    raft: Arc<rocksdb_raft::app::App>
}

#[derive(Serialize, Deserialize)]
struct LookupUrlResponse {
    location: String,
}

#[derive(Serialize, Deserialize, Validate)]
struct CreateShortUrlRequest {
    #[validate(url)]
    long_url: String,
}

#[derive(Serialize, Deserialize)]
struct CreateShortUrlResponse {
    short_url: String,
}


const APP_TYPE_JSON: &str = "application/json";
// raft: &Arc<openraft::Raft<TypeConfig>>
async fn get_leader_info(shared_state: &web::Data<AppStateWithCounter>,) -> Result<(NodeId, Node), actix_web::Error> {
    let metrics = shared_state.raft.raft.metrics().borrow().clone();
    let leader_id = metrics.current_leader.ok_or_else(|| {
        actix_web::error::ErrorInternalServerError("No leader elected yet")
    })?;

    let leader_node = metrics.membership_config.nodes()
        .find(|(id, _)| **id == leader_id)
        .map(|(_, node)| node.clone())
        .ok_or_else(|| {
            actix_web::error::ErrorInternalServerError(format!("Leader node {} not found in membership", leader_id))
        })?;

    Ok((leader_id, leader_node))
}


#[post("/submit")]
async fn create_short_url(
    request_json_bytes: web::Bytes,
    shared_state: web::Data<AppStateWithCounter>,
) -> impl Responder {
    // Parse the incoming JSON.
    let req: CreateShortUrlRequest = match serde_json::from_slice(&request_json_bytes) {
        Ok(req) => req,
        Err(parse_err) => return HttpResponse::from_error(parse_err),
    };

    // Compute hash and validate URL.
    let hash = calculate_hash(&req.long_url);
    let url_str = match req.validate() {
        Ok(_) => req.long_url.to_string(),
        Err(validation_err) => {
            return HttpResponse::BadRequest().json(validation_err)
        }
    };

    // Construct the Raft entry.
    let entry = LongUrlEntry::new(hash, url_str.clone(), shared_state.raft.id);
    tracing::debug!("Creating");
    // Submit the entry to Raft.

    match get_leader_info(&shared_state).await {
        Ok((leader_id, leader)) => {
            if shared_state.raft.id == leader_id {
                // try to write the entry -- write_entry_to_raft still handles races internally,
                // but we don't expect them so we're good to action the write
                write_entry_to_raft(&shared_state, &req, hash, url_str, entry).await.unwrap_or_else(|value| value)
            } else {
                // forward to the node we currently think is leader
                forward_request_to_target(&req, leader).await
            }
        }
        Err(e) => {
            tracing::info!("Failed to look up leader: {}", e);
            HttpResponse::InternalServerError().body("Could not look up leader")
        }
    }

}

async fn write_entry_to_raft(shared_state: &Data<AppStateWithCounter>, req: &CreateShortUrlRequest, hash: u64, url_str: String, entry: LongUrlEntry) -> Result<HttpResponse, HttpResponse> {
    Ok(match shared_state.raft.raft.client_write(entry).await {
        Ok(raft_resp) => {
            tracing::debug!("raft resp: {:?}", raft_resp);
            shared_state.long_url_lookup.insert(hash, url_str);
            let resp = CreateShortUrlResponse { short_url: format!("{:x}", hash) };
            HttpResponse::Ok().content_type(APP_TYPE_JSON).json(resp)
        }
        Err(e) => {
            tracing::debug!("Forwarding to leader");
            // If a forward-to-leader error exists, forward the request.
            if let Some(_forward_info) = e.forward_to_leader() {
                return Ok(forward_request_to_leader(req, shared_state).await);
            } else {
                HttpResponse::InternalServerError().body("Not a ForwardToLeader error")
            }
        }
    })
}

async fn forward_request_to_leader(
    req: &CreateShortUrlRequest,
    shared_state: &Data<AppStateWithCounter>,
) -> HttpResponse {
    match get_leader_info(shared_state).await {
        Ok((_leader_id, leader)) => {
            forward_request_to_target(req, leader).await
        }
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("Failed to forward request: {}", e))
        }
    }
}


/// Forwards the original request to the leader node and returns its response.
async fn forward_request_to_target(
    req: &CreateShortUrlRequest,
    target: BasicNode
) -> HttpResponse {
    // Retrieve the leader's address from the membership metrics.
    let client = reqwest::Client::new();
    let leader_url = format!("http://{}", target.addr);
    let forward_endpoint = format!("{}/submit", leader_url);
    match client.post(&forward_endpoint).json(req).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<CreateShortUrlResponse>().await {
                    Ok(json) => {
                        HttpResponse::Ok().content_type(APP_TYPE_JSON).json(json)
                    }
                    Err(e) => {
                        HttpResponse::InternalServerError().body(format!("Failed to forward request to leader: {}", e))
                    }
                }

            } else {
                HttpResponse::InternalServerError().body("Failed to forward request to leader")
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("Failed to forward request: {}", e)),
    }
}



#[get("/lookup/{hash}")]
async fn lookup_url<'a>(
    from_path: web::Path<String>,
    shared_state: web::Data<AppStateWithCounter>
) -> impl Responder {

    let to_hash = u64::from_str_radix(&from_path, 16);
    if let Err(e) = to_hash {
        tracing::error!("breeeeehhhhhhhhhhhhhhh");
        return HttpResponse::from_error(ShortenerErr::JsonError2(e));
    }
    let hash = to_hash.expect("already checked");
    // if let Some(long_url) = shared_state.long_url_lookup.get(&hash) {
    //     let resp = LookupUrlResponse{ location: long_url};
    //     return HttpResponse::Ok()
    //         .content_type(APP_TYPE_JSON)
    //         .json(resp)
    // }
    let hash_str = hash.to_string();

    let guard = shared_state.raft.key_values.read().await;
    let from_kvs = guard.get(&hash_str);
    // let from_kvs = shared_state.rocks_app.get_entry(hash);
    if let Some(long_url) = from_kvs {
        let resp = LookupUrlResponse{ location: long_url.clone()};
        return HttpResponse::Ok()
            .content_type(APP_TYPE_JSON)
            .json(resp)
    }
    tracing::error!("not found {}", hash);
    HttpResponse::NotFound().finish()
}

#[get("/{hash}")]
async fn redirect(
    hash: web::Path<String>,
    shared_state: web::Data<AppStateWithCounter>
) -> impl Responder {
    let to_hash = u64::from_str_radix(&hash, 16);
    if let Err(e) = to_hash {
        return HttpResponse::from_error(ShortenerErr::JsonError2(e));
    }

    let hash = to_hash.expect("already checked");
    match shared_state.long_url_lookup.get(&hash) {
        None => {HttpResponse::NotFound().finish()}
        Some(long_url) => {
            HttpResponse::MovedPermanently()
                .append_header((header::LOCATION, long_url))
                .append_header((header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, proxy-revalidate"))
                .finish()
        }
    }
}

#[get("/hello")]
async fn hello() -> impl Responder {
    HttpResponse::Ok()
        .content_type(APP_TYPE_JSON)
        .json(Hello {})
}


#[post("/raft/append_entries")]
async fn append_entries(req: Json<AppendEntriesRequest<TypeConfig>>,
                        shared_state: web::Data<AppStateWithCounter>) -> impl Responder {
    shared_state.raft.raft.append_entries(req.0)
        .await
        .map_err(ShortenerErr::RaftError)
        .map(|resp| HttpResponse::Ok().json(resp))

}

#[post("/raft/install_snapshot")]
async fn install_snapshot(req: Json<InstallSnapshotRequest<TypeConfig>>,
                        shared_state: web::Data<AppStateWithCounter>) -> impl Responder {
    shared_state.raft.raft.install_snapshot(req.0)
        .await
        .map_err(ShortenerErr::RaftError2)
        .map(|resp| HttpResponse::Ok().json(resp))
}

#[post("/raft/vote")]
async fn vote(req: Json<VoteRequest<u64>>, shared_state: web::Data<AppStateWithCounter>) -> impl Responder {
    shared_state.raft.raft.vote(req.0)
        .await
        .map_err(ShortenerErr::RaftError)
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt().with_max_level(Level::WARN).init();

    // Create your NoopRaftNetwork instance early on.
    let mut raft_network = rocksdb_raft::network::callback_network_impl::CallbackRaftNetwork::new();

    // Create the Raft node (which uses the network)
    let raft_app = start_raft_node(
        args.node_id,
        format!("{}.db", args.raft_rpc_addr),
        args.http_addr.clone(),
        args.raft_rpc_addr.clone(),
        Arc::new(raft_network.clone()))
        .await;

    let cache = web::Data::new(AppStateWithCounter {
        long_url_lookup: Cache::new(1_000_000),
        raft: raft_app,
    });

    // Spawn the HTTP server in a background task.
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::JsonConfig::default().limit(100 * 1024 * 1024)) // Set payload limit to 10 MB
            .app_data(cache.clone())
            .service(create_short_url)
            .service(lookup_url)
            .service(hello)
            .service(redirect)
            .service(append_entries)
            .service(install_snapshot)
            .service(vote)
            .service(add_learner)
            .service(change_membership)
    })
        .bind(args.http_addr.clone())?
        .run();

    // Spawn the server without awaiting immediately.
    let server_handle = actix_web::rt::spawn(server);

    // Now that the server is online (or being started), create the client handle.
    let raft_client = network::management_rpc_client::RaftManagementRPCClient::new(args.http_addr.clone());

    // Inject the client callbacks into your network implementation.
    // For example, if NoopRaftNetwork has a setter:
    raft_network.set_callbacks(raft_client);

    // Continue with the rest of your program.
    // Await the server task so that your application keeps running.
    server_handle.await??;
    Ok(())
}

fn calculate_hash(t: &str) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}