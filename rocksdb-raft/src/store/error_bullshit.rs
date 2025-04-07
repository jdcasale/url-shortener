use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

/// A custom error wrapper that wraps a Box<dyn Error + Send + Sync>.
#[derive(Debug)]
pub struct CustomAnyError {
    inner: Box<dyn StdError + Send + Sync>,
}

impl Display for CustomAnyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl StdError for CustomAnyError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source()
    }
}

impl From<Box<dyn StdError + Send + Sync>> for CustomAnyError {
    fn from(e: Box<dyn StdError + Send + Sync>) -> Self {
        CustomAnyError { inner: e }
    }
}
