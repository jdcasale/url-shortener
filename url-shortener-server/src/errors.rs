use std::num::ParseIntError;
use actix_web::{HttpResponse, ResponseError};
use openraft::error::InstallSnapshotError;
use serde_json::error::Error as SerdeError;
use thiserror::Error;
use rocksdb_raft::typ::RaftError;


#[derive(Debug, Error)]
pub enum ShortenerErr {
    #[error("Failed to parse JSON")]
    JsonError(#[from] SerdeError),

    #[error("Failed to parse int from hash")]
    HashParsingError(#[from] ParseIntError),

    #[error("An error occurred when modifying Raft state")]
    RaftError(#[from] RaftError),

    #[error("An error occurred when replicating state")]
    RaftSnapshotError(#[from] RaftError<InstallSnapshotError>),
}

impl ResponseError for ShortenerErr {
    fn error_response(&self) -> HttpResponse {
        match self {
            ShortenerErr::JsonError(_) => HttpResponse::BadRequest().body(self.to_string()),
            ShortenerErr::HashParsingError(_) => HttpResponse::UnprocessableEntity().body(self.to_string()),
            ShortenerErr::RaftError(_) => {HttpResponse::InternalServerError().body(self.to_string())}
            ShortenerErr::RaftSnapshotError(_) => {HttpResponse::InternalServerError().body(self.to_string())}
        }
    }
}


