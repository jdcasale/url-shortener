extern crate core;

mod errors;
mod args;
mod services;

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use quick_cache::sync::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use clap::Parser;
use rocksdb_raft::{network, start_raft_node};
use tracing::Level;
use crate::args::Args;
use crate::services::raft_service::{add_learner, append_entries, change_membership, install_snapshot, vote};
use crate::services::shortener_service::{create_short_url, lookup_url, redirect, AppStateWithCounter, APP_TYPE_JSON};

#[derive(Serialize, Deserialize)]
struct Hello {}

#[get("/hello")]
async fn hello() -> impl Responder {
    HttpResponse::Ok()
        .content_type(APP_TYPE_JSON)
        .json(Hello {})
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Create your NoopRaftNetwork instance early on.
    let mut raft_network = rocksdb_raft::network::callback_network_impl::CallbackRaftNetwork::new();

    // Create the Raft node (which uses the network)
    let raft_app = start_raft_node(
        args.node_id,
        format!("{}.db", args.http_addr),
        args.http_addr.clone(),
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
            .service(hello)
            .service(create_short_url)
            .service(lookup_url)
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
    // Await the server task so that your application keeps running.
    server_handle.await??;

    // Now that the server is online, create the client handle.
    let raft_client = network::management_rpc_client::RaftManagementRPCClient::new(args.http_addr.clone());

    // Inject the client callbacks into your network implementation.
    // For example, if NoopRaftNetwork has a setter:
    raft_network.set_callbacks(raft_client);

    Ok(())
}
