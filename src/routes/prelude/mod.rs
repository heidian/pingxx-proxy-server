mod serializers;
use crate::core::{ChargeError, OrderError, RefundError};
use crate::utils::DBError;

impl From<DBError> for OrderError {
    fn from(e: DBError) -> Self {
        match e {
            DBError::SQLFailed(msg) => OrderError::Unexpected(msg),
            DBError::DoesNotExist(msg) => OrderError::BadRequest(msg),
        }
    }
}

impl From<DBError> for ChargeError {
    fn from(e: DBError) -> Self {
        match e {
            DBError::SQLFailed(msg) => ChargeError::InternalError(msg),
            DBError::DoesNotExist(msg) => ChargeError::MalformedRequest(msg),
        }
    }
}

impl From<DBError> for RefundError {
    fn from(e: DBError) -> Self {
        match e {
            DBError::SQLFailed(msg) => RefundError::Unexpected(msg),
            DBError::DoesNotExist(msg) => RefundError::BadRequest(msg),
        }
    }
}
