use std::num::ParseIntError;
use actix_web::{HttpResponse, ResponseError};
use openraft::error::InstallSnapshotError;
use serde_json::error::Error as SerdeError;
use thiserror::Error;
use rocksdb_raft::typ::RaftError;


#[derive(Debug, Error)]
pub enum ShortenerErr {
    #[error("Failed to parse JSON")]
    Json(#[from] SerdeError),

    #[error("Failed to parse int from hash")]
    HashParsing(#[from] ParseIntError),

    #[error("An error occurred when modifying Raft state")]
    Raft(#[from] RaftError),

    #[error("An error occurred when replicating state")]
    RaftSnapshot(#[from] RaftError<InstallSnapshotError>),
}

impl ResponseError for ShortenerErr {
    fn error_response(&self) -> HttpResponse {
        match self {
            ShortenerErr::Json(_) => HttpResponse::BadRequest().body(self.to_string()),
            ShortenerErr::HashParsing(_) => HttpResponse::UnprocessableEntity().body(self.to_string()),
            ShortenerErr::Raft(_) => {HttpResponse::InternalServerError().body(self.to_string())}
            ShortenerErr::RaftSnapshot(_) => {HttpResponse::InternalServerError().body(self.to_string())}
        }
    }
}


