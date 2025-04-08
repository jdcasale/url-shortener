use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use actix_web::{get, post, web, HttpResponse, Responder};
use actix_web::http::header;
use actix_web::web::Data;
use openraft::BasicNode;
use quick_cache::sync::Cache;
use serde::{Deserialize, Serialize};
use validator::Validate;
use rocksdb_raft::network::callback_network_impl::{Node, NodeId};
use rocksdb_raft::store::types::LongUrlEntry;
use crate::errors::ShortenerErr;

pub const APP_TYPE_JSON: &str = "application/json";

pub struct AppStateWithCounter {
    pub(crate) long_url_lookup: Cache<u64, String>,
    pub(crate) raft: Arc<rocksdb_raft::app::App>
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

fn calculate_hash(t: &str) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

async fn get_leader_info(shared_state: &Data<AppStateWithCounter>,) -> Result<(NodeId, Node), actix_web::Error> {
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
    let client = reqwest::Client::default();
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
    shared_state: Data<AppStateWithCounter>
) -> impl Responder {

    let to_hash = u64::from_str_radix(&from_path, 16);
    if let Err(e) = to_hash {
        return HttpResponse::from_error(ShortenerErr::HashParsing(e));
    }
    let hash = to_hash.expect("already checked");
    // if let Some(long_url) = shared_state.long_url_lookup.get(&hash) {
    //     let resp = LookupUrlResponse{ location: long_url};
    //     return HttpResponse::Ok()
    //         .content_type(APP_TYPE_JSON)
    //         .json(resp)
    // }
    let hash_str = hash.to_string();

    let recent = shared_state.raft.new_writes_kvs.read().await;

    let from_recent = recent.get(&hash_str);
    // let from_kvs = shared_state.rocks_app.get_entry(hash);
    if let Some(long_url) = from_recent {
        let resp = LookupUrlResponse{ location: long_url.clone()};
        return HttpResponse::Ok()
            .content_type(APP_TYPE_JSON)
            .json(resp)
    }
    let history = shared_state.raft.historical_kvs.read().await;
    let from_history = history.get(&hash_str);
    // let from_kvs = shared_state.rocks_app.get_entry(hash);
    if let Some(long_url) = from_history {
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
        return HttpResponse::from_error(ShortenerErr::HashParsing(e));
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