//! TaskQueue: runs a plan with bounded concurrency, progress events,
//! cancellation and isolated failures.

use crate::cancel::{CancelToken, TaskCancel};
use crate::error::{EngineError, ErrorCode};
use crate::executors;
use crate::format::FileFormat;
use crate::limits::Limits;
use crate::operation::{OperationRequest, Preset};
use crate::output::{OutputManager, size_on_disk};
use crate::plan::{Plan, TaskNote, TaskSpec};
use crate::tools::ToolSet;
use crate::validator;
use serde::Serialize;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutputFile {
    pub id: String,
    #[serde(skip)]
    pub path: PathBuf,
    pub name: String,
    pub display_path: String,
    pub is_dir: bool,
    #[ts(type = "number")]
    pub size: u64,
    /// Published under a numbered name because the desired one was taken.
    pub renamed: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export)]
pub enum TaskOutcome {
    Succeeded {
        outputs: Vec<OutputFile>,
        notes: Vec<TaskNote>,
    },
    Failed {
        error: EngineError,
    },
    /// Not processed by policy (for example, the output already existed).
    Skipped {
        error: EngineError,
    },
    Cancelled,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobSummary {
    pub succeeded: u32,
    pub failed: u32,
    pub skipped: u32,
    pub cancelled: u32,
    pub outputs: Vec<OutputFile>,
    #[ts(type = "number")]
    pub duration_ms: u64,
    #[ts(type = "number")]
    pub input_bytes: u64,
    #[ts(type = "number")]
    pub output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum JobEvent {
    #[serde(rename_all = "camelCase")]
    JobStarted { job_id: String, total: u32 },
    #[serde(rename_all = "camelCase")]
    TaskStarted { job_id: String, task_id: String },
    #[serde(rename_all = "camelCase")]
    TaskProgress {
        job_id: String,
        task_id: String,
        progress: f32,
    },
    #[serde(rename_all = "camelCase")]
    TaskFinished {
        job_id: String,
        task_id: String,
        outcome: TaskOutcome,
    },
    #[serde(rename_all = "camelCase")]
    JobFinished { job_id: String, summary: JobSummary },
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: JobEvent);
}

impl<F: Fn(JobEvent) + Send + Sync> EventSink for F {
    fn emit(&self, event: JobEvent) {
        self(event)
    }
}

/// Cancellation handles for a running job.
#[derive(Debug, Default)]
pub struct JobHandle {
    job: CancelToken,
    tasks: HashMap<String, CancelToken>,
}

impl JobHandle {
    pub fn new(plan: &Plan) -> Self {
        Self {
            job: CancelToken::new(),
            tasks: plan
                .tasks
                .iter()
                .map(|t| (t.id.clone(), CancelToken::new()))
                .collect(),
        }
    }

    pub fn cancel_all(&self) {
        self.job.cancel();
    }

    /// Cancels one task. Returns false for unknown ids.
    pub fn cancel_task(&self, task_id: &str) -> bool {
        match self.tasks.get(task_id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    fn task_cancel(&self, task_id: &str) -> TaskCancel {
        TaskCancel::new(
            self.job.clone(),
            self.tasks.get(task_id).cloned().unwrap_or_default(),
        )
    }
}

/// Everything an executor needs while running one task.
pub struct TaskContext<'a> {
    pub cancel: TaskCancel,
    pub tools: &'a ToolSet,
    pub limits: &'a Limits,
    pub output: &'a OutputManager,
    progress: &'a (dyn Fn(f32) + Sync),
}

impl<'a> TaskContext<'a> {
    pub fn new(
        cancel: TaskCancel,
        tools: &'a ToolSet,
        limits: &'a Limits,
        output: &'a OutputManager,
        progress: &'a (dyn Fn(f32) + Sync),
    ) -> Self {
        Self {
            cancel,
            tools,
            limits,
            output,
            progress,
        }
    }

    /// Reports progress in `0.0..=1.0` for the current task.
    pub fn progress(&self, value: f32) {
        (self.progress)(value.clamp(0.0, 1.0));
    }
}

/// How many tasks of this kind may run at the same time.
pub fn concurrency(request: &OperationRequest) -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    match request {
        // The AVIF encoder is itself multi-threaded and memory hungry.
        OperationRequest::ImageConvert(o) if o.target == FileFormat::Avif => 2.min(cpus),
        OperationRequest::ImageConvert(_) => cpus.clamp(1, 4),
        OperationRequest::PdfToImages(_) => cpus.clamp(1, 2),
        OperationRequest::PdfMerge(_)
        | OperationRequest::PdfSplit(_)
        | OperationRequest::PdfRotate(_) => 2,
        OperationRequest::AudioConvert(_) => cpus.clamp(1, 2),
        // FFmpeg already uses every core for video.
        OperationRequest::VideoConvert(_) => 1,
        OperationRequest::ArchiveCreate(_) | OperationRequest::ArchiveExtract(_) => 1,
    }
}

const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);
const PROGRESS_JUMP: f32 = 0.1;

/// Runs every task of `plan`, emitting events, and returns the summary.
/// One failing task never stops the others.
pub fn run_job(
    plan: &Plan,
    handle: &JobHandle,
    tools: &ToolSet,
    limits: &Limits,
    sink: &dyn EventSink,
) -> JobSummary {
    let started = Instant::now();
    let job_id = plan.job_id.clone();
    sink.emit(JobEvent::JobStarted {
        job_id: job_id.clone(),
        total: plan.tasks.len() as u32,
    });

    let output = OutputManager::new(
        plan.output_settings.collision,
        plan.tasks
            .iter()
            .flat_map(|t| t.inputs.iter().map(|i| i.path.as_path())),
    );
    let summary = Mutex::new(JobSummary::default());
    let next = AtomicUsize::new(0);
    let workers = concurrency(&plan.request).min(plan.tasks.len()).max(1);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(task) = plan.tasks.get(index) else {
                        break;
                    };
                    let outcome = run_task(plan, task, handle, tools, limits, &output, sink);
                    {
                        let mut summary = summary.lock().expect("summary lock");
                        summary.input_bytes += task.inputs.iter().map(|i| i.size).sum::<u64>();
                        match &outcome {
                            TaskOutcome::Succeeded { outputs, .. } => {
                                summary.succeeded += 1;
                                summary.output_bytes += outputs.iter().map(|o| o.size).sum::<u64>();
                                summary.outputs.extend(outputs.iter().cloned());
                            }
                            TaskOutcome::Failed { .. } => summary.failed += 1,
                            TaskOutcome::Skipped { .. } => summary.skipped += 1,
                            TaskOutcome::Cancelled => summary.cancelled += 1,
                        }
                    }
                    sink.emit(JobEvent::TaskFinished {
                        job_id: plan.job_id.clone(),
                        task_id: task.id.clone(),
                        outcome,
                    });
                }
            });
        }
    });

    let mut summary = summary.into_inner().expect("summary lock");
    summary.duration_ms = started.elapsed().as_millis() as u64;
    sink.emit(JobEvent::JobFinished {
        job_id,
        summary: summary.clone(),
    });
    summary
}

fn run_task(
    plan: &Plan,
    task: &TaskSpec,
    handle: &JobHandle,
    tools: &ToolSet,
    limits: &Limits,
    output: &OutputManager,
    sink: &dyn EventSink,
) -> TaskOutcome {
    let cancel = handle.task_cancel(&task.id);
    if cancel.is_cancelled() {
        return TaskOutcome::Cancelled;
    }
    sink.emit(JobEvent::TaskStarted {
        job_id: plan.job_id.clone(),
        task_id: task.id.clone(),
    });

    let last_emit = Mutex::new((
        Instant::now()
            .checked_sub(PROGRESS_INTERVAL)
            .unwrap_or_else(Instant::now),
        0.0f32,
    ));
    // Fine-grained updates are rate limited; visible jumps always go out so
    // a long single step (an AVIF encode) never looks stuck at 0%.
    let report = |value: f32| {
        let mut last = last_emit.lock().expect("progress lock");
        let (at, previous) = *last;
        if value >= 1.0 || value - previous >= PROGRESS_JUMP || at.elapsed() >= PROGRESS_INTERVAL {
            *last = (Instant::now(), value);
            sink.emit(JobEvent::TaskProgress {
                job_id: plan.job_id.clone(),
                task_id: task.id.clone(),
                progress: value,
            });
        }
    };
    let ctx = TaskContext::new(cancel, tools, limits, output, &report);

    let result =
        catch_unwind(AssertUnwindSafe(|| execute(plan, task, &ctx))).unwrap_or_else(|panic| {
            let message = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".into());
            Err(EngineError::internal(format!(
                "executor panicked: {message}"
            )))
        });

    match result {
        Ok((outputs, notes)) => TaskOutcome::Succeeded { outputs, notes },
        Err(error) if error.is_cancelled() => TaskOutcome::Cancelled,
        Err(error) if error.code == ErrorCode::OutputExists => TaskOutcome::Skipped { error },
        Err(error) => {
            log::warn!("task failed: {:?} {}", error.code, error.detail);
            TaskOutcome::Failed { error }
        }
    }
}

fn execute(
    plan: &Plan,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> Result<(Vec<OutputFile>, Vec<TaskNote>), EngineError> {
    ctx.cancel.check()?;
    ctx.progress(0.0);
    let produced = executors::produce(&plan.request, task, ctx)?;
    ctx.cancel.check()?;
    ctx.progress(0.96);
    validator::validate(produced.staged.path(), &produced.expectation, ctx.tools)?;
    // Last chance to cancel: nothing has been published yet.
    ctx.cancel.check()?;
    let published = ctx
        .output
        .publish(produced.staged, &task.output().desired_path())?;
    ctx.progress(1.0);
    let name = published
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let file = OutputFile {
        id: uuid::Uuid::new_v4().to_string(),
        display_path: published.path.to_string_lossy().into_owned(),
        is_dir: published.path.is_dir(),
        size: size_on_disk(&published.path),
        name,
        path: published.path,
        renamed: published.renamed,
    };
    Ok((vec![file], produced.notes))
}

/// Default quality (1-100) for lossy encoders.
pub fn preset_quality(format: FileFormat, preset: Preset) -> u8 {
    match (format, preset) {
        (FileFormat::Avif, Preset::Smallest) => 50,
        (FileFormat::Avif, Preset::Quality) => 82,
        (FileFormat::Avif, _) => 68,
        (_, Preset::Smallest) => 60,
        (_, Preset::Quality) => 92,
        (_, _) => 80,
    }
}
