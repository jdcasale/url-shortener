use std::num::ParseIntError;
use actix_web::{HttpResponse, ResponseError};
use openraft::error::InstallSnapshotError;
use serde_json::error::Error as SerdeError;
use thiserror::Error;
use url::ParseError;
use rocksdb_raft::typ::RaftError;
// pub type Result<T> = std::result::Result<T, ShortenerErr>;


#[derive(Debug, Error)]
pub enum ShortenerErr {
    #[error("Failed to parse JSON")]
    JsonError(#[from] SerdeError),

    #[error("Failed to parse JSON")]
    JsonError2(#[from] ParseIntError),

    #[error("Failed to parse URL")]
    UrlParseError(#[from] ParseError),

    #[error("An unexpected error occurred")]
    UnexpectedError,

    #[error("An error occurred when replicating state")]
    RaftError(#[from] RaftError),

    #[error("An error occurred when replicating state")]
    RaftError2(#[from] RaftError<InstallSnapshotError>),
}

impl ResponseError for ShortenerErr {
    fn error_response(&self) -> HttpResponse {
        match self {
            ShortenerErr::JsonError(_) => HttpResponse::BadRequest().body(self.to_string()),
            ShortenerErr::UnexpectedError => HttpResponse::InternalServerError().body(self.to_string()),
            ShortenerErr::JsonError2(_) => HttpResponse::UnprocessableEntity().body(self.to_string()),
            ShortenerErr::UrlParseError(_) => {HttpResponse::UnprocessableEntity().body(self.to_string())}
            ShortenerErr::RaftError(_) => {HttpResponse::InternalServerError().body(self.to_string())}
            ShortenerErr::RaftError2(_) => {HttpResponse::InternalServerError().body(self.to_string())}
        }
    }
}


