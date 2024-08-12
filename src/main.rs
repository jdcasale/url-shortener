extern crate core;

mod errors;

use actix_web::{post, web, App, HttpResponse, HttpServer, Responder, get};
use quick_cache::sync::{Cache};
use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};
use actix_web::http::header;
use url::Url;
use crate::errors::ShortenerErr;

#[derive(Serialize, Deserialize)]
struct Hello {}

struct AppStateWithCounter {
    long_url_lookup: Cache<u64, String>,
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
    shared_state.long_url_lookup.insert(hash, url_str);
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
    match shared_state.long_url_lookup.get(&hash) {
        None => {HttpResponse::NotFound().finish()}
        Some(long_url) => {
            let resp = LookupUrlResponse{ location: long_url};
            HttpResponse::Ok()
                .content_type(APP_TYPE_JSON)
                .json(resp)
        }
    }
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
                .append_header((header::LOCATION, long_url))//format!("https://{long_url}")))
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


#[actix_web::main]
async fn main() -> std::io::Result<()> {

    let cache = web::Data::new(AppStateWithCounter {
        long_url_lookup: Cache::new(100_000_000)
    });

    HttpServer::new(move || {
        // move counter into the closure
        App::new()
            .app_data(cache.clone()) // <- register the created data
            .service(create_short_url)
            .service(lookup_url)
            .service(redirect)
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