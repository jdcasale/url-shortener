use std::num::ParseIntError;
use actix_web::{HttpResponse, ResponseError};
use serde_json::error::Error as SerdeError;
use thiserror::Error;

// pub type Result<T> = std::result::Result<T, ShortenerErr>;


#[derive(Debug, Error)]
pub enum ShortenerErr {
    #[error("Failed to parse JSON")]
    JsonError(#[from] SerdeError),

    #[error("Failed to parse JSON")]
    JsonError2(#[from] ParseIntError),

    #[error("An unexpected error occurred")]
    UnexpectedError,
}

impl ResponseError for ShortenerErr {
    fn error_response(&self) -> HttpResponse {
        match self {
            ShortenerErr::JsonError(_) => HttpResponse::BadRequest().body(self.to_string()),
            ShortenerErr::UnexpectedError => HttpResponse::InternalServerError().body(self.to_string()),
            ShortenerErr::JsonError2(_) => HttpResponse::InternalServerError().body(self.to_string())
        }
    }
}


