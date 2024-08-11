use std::error::Error;

#[derive(Debug)]
pub struct BookwormError {
    message: String,
}

impl std::fmt::Display for BookwormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl BookwormError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl Error for BookwormError {}

pub type BookwormResult<T> = Result<T, BookwormError>;
