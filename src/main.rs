extern crate core;

mod errors;

use actix_web::{post, web, App, HttpResponse, HttpServer, Responder, get};
use quick_cache::sync::{Cache};
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};
use crate::errors::ShortenerErr;


struct AppStateWithCounter {
    long_url_lookup: Cache<u64, String>,
}

#[derive(Deserialize)]
struct LookupUrlRequest<'a> {
    short_url: &'a str,
}

#[derive(Serialize, Deserialize)]
struct LookupUrlResponse {
    long_url: String,
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
    let hash = calculate_hash(req.long_url);
    shared_state.long_url_lookup.insert(hash, req.long_url.to_string());
    let resp = CreateShortUrlResponse{short_url: format!("{:x}", hash)};
    HttpResponse::Ok()
        .content_type(APP_TYPE_JSON)
        .json(resp)
}


#[get("/s/{hash}")]
async fn lookup_url(
    request_json_bytes: web::Bytes,
    shared_state: web::Data<AppStateWithCounter>
) -> impl Responder {
    let from_request_json: serde_json::Result<LookupUrlRequest> = serde_json::from_slice(
        &request_json_bytes
    );

    if let Err(parse_err) = from_request_json {
        return HttpResponse::from_error(parse_err);
    };
    let req = from_request_json.expect("already checked for errors");
    let to_hash = u64::from_str_radix(req.short_url, 16);
    if let Err(e) = to_hash {
        return HttpResponse::from_error(ShortenerErr::JsonError2(e));
    }
    let hash = to_hash.expect("already checked");
    match shared_state.long_url_lookup.get(&hash) {
        None => {HttpResponse::NotFound().finish()}
        Some(long_url) => {
            let resp = LookupUrlResponse{long_url: format!("{long_url}")};
            HttpResponse::Ok()
                .content_type(APP_TYPE_JSON)
                .json(resp)
        }
    }
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {

    let cache = web::Data::new(AppStateWithCounter {
        long_url_lookup: Cache::new(1_000_000)
    });

    HttpServer::new(move || {
        // move counter into the closure
        App::new()
            .app_data(cache.clone()) // <- register the created data
            .service(create_short_url)
            .service(lookup_url)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

fn calculate_hash(t: &str) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
