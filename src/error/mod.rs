use std::sync::{PoisonError, MutexGuard};

#[derive(Debug)]
pub enum Error {
    SqliteError(sqlite::Error),
    UnknownError,
    PoisonError
}

impl From<sqlite::Error> for Error {
    fn from(value: sqlite::Error) -> Self {
        Self::SqliteError(value)
    }
}

impl<'a,T> From<PoisonError<MutexGuard<'a,T>>> for Error {
    fn from(_: PoisonError<MutexGuard<'a,T>>) -> Self {
        Self::PoisonError
    }
}