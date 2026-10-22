//! OmniDrop conversion engine.
//!
//! The pipeline is: [`inspector`] → [`capability`] → [`plan`] → [`job`]
//! (queue) → [`executors`] → [`validator`] → [`output`] (publish).
//! Nothing in this crate depends on the desktop shell, so every stage can be
//! tested in isolation.

pub mod cancel;
pub mod capability;
pub mod error;
pub mod executors;
pub mod format;
pub mod inspector;
pub mod job;
pub mod limits;
pub mod names;
pub mod operation;
pub mod output;
pub mod pages;
pub mod plan;
pub mod tools;
pub mod validator;

pub use capability::{CapabilityRegistry, ConversionCapability};
pub use error::{EngineError, EngineResult, ErrorCode};
pub use format::{FileFormat, FormatFamily};
pub use inspector::{FileDetails, FileInfo, InputEntry};
pub use job::{JobEvent, JobHandle, JobSummary, OutputFile, TaskOutcome, run_job};
pub use limits::Limits;
pub use operation::{OperationId, OperationRequest, OutputSettings};
pub use plan::{Plan, plan};
pub use tools::ToolSet;
