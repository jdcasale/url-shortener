extern crate core;

mod errors;

use actix_web::{post, web, App, HttpResponse, HttpServer, Responder, get};
use quick_cache::sync::{Cache};
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use actix_web::http::header;
use openraft::EntryPayload;
use url::Url;
use web::Json;
use rocksdb_raft::start_raft_node;
use openraft::raft::{AppendEntriesRequest, InstallSnapshotRequest};
use rocksdb_raft::rocksb_store::{LongUrlEntry};
use crate::errors::ShortenerErr;
use rocksdb_raft::rocksb_store::TypeConfig;

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

#[derive(Deserialize)]
struct CreateShortUrlRequest<'a> {
    long_url: &'a str,
}

#[derive(Serialize, Deserialize)]
struct CreateShortUrlResponse {
    short_url: String,
}


const APP_TYPE_JSON: &'static str = "application/json";

#[post("/submit")]
async fn create_short_url(
    request_json_bytes: web::Bytes,
    shared_state: web::Data<AppStateWithCounter>) -> impl Responder {
    let from_request_json: serde_json::Result<CreateShortUrlRequest> = serde_json::from_slice(
        &request_json_bytes
    );

    if let Err(parse_err) = from_request_json {
        return HttpResponse::from_error(parse_err);
    };

    let req = from_request_json.expect("already checked for errors");

    let maybe_url = validate_url(req.long_url);
    let url_str = match maybe_url {
        Ok(valid_url) => {
            valid_url.to_string()
        }
        Err(url_err) => {
            return HttpResponse::from_error(url_err)
        }
    };

    let hash = calculate_hash(req.long_url);
    let entry = LongUrlEntry::new(hash, url_str.clone(), 1u64);
    // let raft_entry = openraft::Entry {
    //     log_id: Default::default(),
    //     payload: EntryPayload::Normal(entry.clone()),
    // };
    // let append_req = AppendEntriesRequest {
    //     vote: Default::default(),
    //     prev_log_id: None,
    //     entries: vec![raft_entry],
    //     leader_commit: None,
    // };


    let resp = shared_state
        .raft
        .raft
        .client_write(entry)
        .await;
    let _ = resp.unwrap();
    // shared_state.rocks_app.append_entry(entry);
    shared_state.long_url_lookup.insert(hash, url_str.clone());
    let resp = CreateShortUrlResponse{short_url: format!("{:x}", hash)};
    HttpResponse::Ok()
        .content_type(APP_TYPE_JSON)
        .json(resp)
}


#[get("/lookup/{hash}")]
async fn lookup_url<'a>(
    from_path: web::Path<String>,
    shared_state: web::Data<AppStateWithCounter>
) -> impl Responder {

    let to_hash = u64::from_str_radix(&from_path, 16);
    if let Err(e) = to_hash {
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
    return HttpResponse::Ok()
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


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let raft_rpc_addr = "0.0.0.0:21001".to_string();

    let raft_app =  start_raft_node(
        0,
        format!("{}.db", raft_rpc_addr),
        raft_rpc_addr.clone(),
        raft_rpc_addr.clone())
        .await;
    let cache = web::Data::new(AppStateWithCounter {
        long_url_lookup: Cache::new(100_000_000),
        raft: raft_app
    });


    HttpServer::new(move || {
        // move counter into the closure
        App::new()
            .app_data(cache.clone())
            .service(create_short_url)
            .service(lookup_url)
            .service(redirect)
            .service(append_entries)
            .service(install_snapshot)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

fn calculate_hash(t: &str) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

// TODO(@jdcasale): obv use a real library for this
fn validate_url(url: &str) -> Result<Url, ShortenerErr> {
    let abs_url = if !url.starts_with("http://") && !url.starts_with("https://"){
        format!("https://{url}")
    } else {
        url.to_string()
    };
    match Url::parse(&abs_url) {
        Ok(parsed_url) => {
            // Additional checks can be added here if needed
            Ok(parsed_url)
        }
        Err(e) => {
            Err(ShortenerErr::UrlParseError(e))
        }
    }
}