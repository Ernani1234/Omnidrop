//! ConversionExecutors: one module per engine. Each produces a staged result
//! plus what the validator should expect from it; publishing is done by the
//! job runner.

pub mod archive;
pub mod image;
pub mod media;
pub mod pdf;

use crate::error::EngineResult;
use crate::job::TaskContext;
use crate::operation::OperationRequest;
use crate::output::Staged;
use crate::plan::{TaskNote, TaskSpec};
use crate::validator::Expectation;

pub struct Produced {
    pub staged: Staged,
    pub expectation: Expectation,
    pub notes: Vec<TaskNote>,
}

pub fn produce(
    request: &OperationRequest,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> EngineResult<Produced> {
    match request {
        OperationRequest::ImageConvert(options) => image::convert(options, task, ctx),
        OperationRequest::PdfMerge(_) => pdf::merge(task, ctx),
        OperationRequest::PdfSplit(options) => pdf::split(options, task, ctx),
        OperationRequest::PdfRotate(options) => pdf::rotate(options, task, ctx),
        OperationRequest::PdfToImages(options) => pdf::to_images(options, task, ctx),
        OperationRequest::AudioConvert(options) => media::convert_audio(options, task, ctx),
        OperationRequest::VideoConvert(options) => media::convert_video(options, task, ctx),
        OperationRequest::ArchiveCreate(options) => archive::create(options, task, ctx),
        OperationRequest::ArchiveExtract(_) => archive::extract(task, ctx),
    }
}
