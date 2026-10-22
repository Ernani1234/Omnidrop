use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use ts_rs::TS;

/// Stable, machine-readable error categories.
///
/// The frontend maps each code to plain-language copy and a suggested action.
/// `EngineError::detail` carries technical context for diagnostics only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ErrorCode {
    NotFound,
    PermissionDenied,
    Unreadable,
    UnsupportedFormat,
    UnsupportedConversion,
    InputTooLarge,
    DimensionsTooLarge,
    TargetDimensionsUnsupported,
    PdfEncrypted,
    InvalidPageRange,
    ArchiveEncrypted,
    ArchiveLimitExceeded,
    ToolMissing,
    ToolFailed,
    Timeout,
    DestinationNotWritable,
    DiskFull,
    OutputExists,
    ValidationFailed,
    Cancelled,
    InvalidOptions,
    NotEnoughFiles,
    Internal,
}

/// Structured error: a code for the UI plus a technical detail for logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EngineError {
    pub code: ErrorCode,
    pub detail: String,
}

pub type EngineResult<T> = Result<T, EngineError>;

impl EngineError {
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn cancelled() -> Self {
        Self::new(ErrorCode::Cancelled, "cancelled by user")
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, detail)
    }

    pub fn invalid_options(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidOptions, detail)
    }

    /// Maps an I/O error that happened while reading an input file.
    pub fn read(err: &io::Error) -> Self {
        let code = match err.kind() {
            io::ErrorKind::NotFound => ErrorCode::NotFound,
            io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
            _ => ErrorCode::Unreadable,
        };
        Self::new(code, format!("read error: {err}"))
    }

    /// Maps an I/O error that happened while writing an output.
    pub fn write(err: &io::Error) -> Self {
        let code = match err.kind() {
            io::ErrorKind::StorageFull
            | io::ErrorKind::QuotaExceeded
            | io::ErrorKind::FileTooLarge => ErrorCode::DiskFull,
            io::ErrorKind::PermissionDenied
            | io::ErrorKind::ReadOnlyFilesystem
            | io::ErrorKind::NotFound => ErrorCode::DestinationNotWritable,
            _ => ErrorCode::Internal,
        };
        Self::new(code, format!("write error: {err}"))
    }

    pub fn is_cancelled(&self) -> bool {
        self.code == ErrorCode::Cancelled
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.detail)
    }
}

impl std::error::Error for EngineError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_io_errors_by_direction() {
        let missing = io::Error::from(io::ErrorKind::NotFound);
        assert_eq!(EngineError::read(&missing).code, ErrorCode::NotFound);
        assert_eq!(
            EngineError::write(&missing).code,
            ErrorCode::DestinationNotWritable
        );
        let full = io::Error::from(io::ErrorKind::StorageFull);
        assert_eq!(EngineError::write(&full).code, ErrorCode::DiskFull);
    }

    #[test]
    fn serializes_with_camel_case_code() {
        let err = EngineError::new(ErrorCode::PdfEncrypted, "x");
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"code":"pdfEncrypted","detail":"x"}"#);
    }
}
