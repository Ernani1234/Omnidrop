//! Audio and video through the FFmpeg installed on the computer.
//!
//! Arguments are passed as a vector (no shell), inputs use the `file:`
//! protocol, and the process is killed on cancellation or timeout.

use super::Produced;
use crate::error::{EngineError, EngineResult, ErrorCode};
use crate::format::{FileFormat, FormatFamily};
use crate::inspector::FileDetails;
use crate::job::TaskContext;
use crate::operation::{AudioConvertOptions, Preset, VideoConvertOptions};
use crate::output::Staged;
use crate::plan::TaskSpec;
use crate::tools::{ffmpeg_file_arg, run_capture, tool_command};
use crate::validator::Expectation;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const STDERR_TAIL: usize = 4096;

/// Reads duration and streams with ffprobe.
pub fn probe(ffprobe: &Path, path: &Path) -> EngineResult<FileDetails> {
    let args: Vec<OsString> = vec![
        "-v".into(),
        "error".into(),
        "-print_format".into(),
        "json".into(),
        "-show_format".into(),
        "-show_streams".into(),
        ffmpeg_file_arg(path),
    ];
    let output = run_capture(ffprobe, args, Duration::from_secs(30))
        .map_err(|e| EngineError::new(ErrorCode::ToolFailed, format!("ffprobe: {e}")))?;
    if !output.success {
        return Err(EngineError::new(
            ErrorCode::Unreadable,
            format!("ffprobe: {}", output.stderr.trim()),
        ));
    }
    parse_probe(&output.stdout)
}

pub fn parse_probe(json: &str) -> EngineResult<FileDetails> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| EngineError::new(ErrorCode::Unreadable, format!("ffprobe output: {e}")))?;
    let streams = value["streams"].as_array().cloned().unwrap_or_default();
    let is_cover = |s: &serde_json::Value| s["disposition"]["attached_pic"].as_i64() == Some(1);
    let video = streams
        .iter()
        .find(|s| s["codec_type"] == "video" && !is_cover(s));
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    if video.is_none() && audio.is_none() {
        return Err(EngineError::new(
            ErrorCode::Unreadable,
            "no audio or video streams",
        ));
    }
    let duration_ms = value["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d >= 0.0)
        .map(|d| (d * 1000.0).round() as u64);
    let dim = |key: &str| video.and_then(|v| v[key].as_u64()).map(|v| v as u32);
    let codec =
        |s: Option<&serde_json::Value>| s.and_then(|s| s["codec_name"].as_str()).map(String::from);
    Ok(FileDetails::Media {
        duration_ms,
        has_video: video.is_some(),
        has_audio: audio.is_some(),
        width: dim("width"),
        height: dim("height"),
        video_codec: codec(video),
        audio_codec: codec(audio),
    })
}

/// Arguments that encode the audio track as `target`.
pub fn audio_args(target: FileFormat, preset: Preset) -> Vec<String> {
    let bitrate = |small: &str, balanced: &str, best: &str| match preset {
        Preset::Smallest => small.to_string(),
        Preset::Quality => best.to_string(),
        _ => balanced.to_string(),
    };
    let mut args: Vec<String> = vec![
        "-vn".into(),
        "-sn".into(),
        "-dn".into(),
        "-map".into(),
        "0:a:0".into(),
    ];
    match target {
        FileFormat::Mp3 => {
            args.extend(["-c:a", "libmp3lame", "-b:a"].map(String::from));
            args.push(bitrate("128k", "192k", "320k"));
            args.extend(["-f", "mp3"].map(String::from));
        }
        FileFormat::M4a => {
            args.extend(["-c:a", "aac", "-b:a"].map(String::from));
            args.push(bitrate("128k", "192k", "256k"));
            args.extend(["-movflags", "+faststart", "-f", "ipod"].map(String::from));
        }
        FileFormat::Flac => {
            args.extend(["-c:a", "flac", "-compression_level"].map(String::from));
            args.push(if preset == Preset::Smallest { "8" } else { "5" }.into());
            args.extend(["-f", "flac"].map(String::from));
        }
        FileFormat::Wav => args.extend(["-c:a", "pcm_s16le", "-f", "wav"].map(String::from)),
        _ => {}
    }
    args
}

/// Arguments that encode video (and audio, unless removed) as `target`.
pub fn video_args(
    target: FileFormat,
    preset: Preset,
    max_height: Option<u32>,
    remove_audio: bool,
    webm_audio: &str,
) -> Vec<String> {
    let pick = |small: &str, balanced: &str, best: &str| match preset {
        Preset::Smallest => small.to_string(),
        Preset::Quality => best.to_string(),
        _ => balanced.to_string(),
    };
    let mut args: Vec<String> = vec!["-map".into(), "0:v:0".into()];
    if !remove_audio {
        args.extend(["-map", "0:a:0?"].map(String::from));
    }
    args.extend(["-sn", "-dn"].map(String::from));
    if let Some(height) = max_height {
        args.push("-vf".into());
        args.push(format!("scale=-2:'min({height},trunc(ih/2)*2)'"));
    }
    match target {
        FileFormat::Webm => {
            args.extend(
                [
                    "-c:v",
                    "libvpx-vp9",
                    "-b:v",
                    "0",
                    "-row-mt",
                    "1",
                    "-deadline",
                    "good",
                    "-crf",
                ]
                .map(String::from),
            );
            args.push(pick("38", "32", "26"));
            args.push("-cpu-used".into());
            args.push(pick("4", "3", "2"));
            if !remove_audio {
                args.extend([
                    "-c:a".to_string(),
                    webm_audio.to_string(),
                    "-b:a".to_string(),
                ]);
                args.push(pick("96k", "128k", "160k"));
            }
            args.extend(["-f", "webm"].map(String::from));
        }
        _ => {
            args.extend(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf"].map(String::from));
            args.push(pick("28", "23", "18"));
            args.push("-preset".into());
            args.push(pick("veryfast", "medium", "slow"));
            if !remove_audio {
                args.extend(["-c:a", "aac", "-b:a"].map(String::from));
                args.push(pick("128k", "160k", "192k"));
            }
            if target == FileFormat::Mkv {
                args.extend(["-f", "matroska"].map(String::from));
            } else {
                args.extend(["-movflags", "+faststart", "-f", "mp4"].map(String::from));
            }
        }
    }
    if remove_audio {
        args.push("-an".into());
    }
    args
}

fn input_duration(details: &Option<FileDetails>) -> Option<Duration> {
    match details {
        Some(FileDetails::Media {
            duration_ms: Some(ms),
            ..
        }) => Some(Duration::from_millis(*ms)),
        _ => None,
    }
}

fn input_has_audio(details: &Option<FileDetails>) -> Option<bool> {
    match details {
        Some(FileDetails::Media { has_audio, .. }) => Some(*has_audio),
        _ => None,
    }
}

pub fn convert_audio(
    options: &AudioConvertOptions,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> EngineResult<Produced> {
    let input = &task.inputs[0];
    let args = audio_args(options.target, options.preset);
    let temp = run_conversion(task, ctx, args, input_duration(&input.details))?;
    Ok(Produced {
        staged: Staged::File(temp),
        expectation: Expectation::Media {
            format: options.target,
            has_video: false,
            has_audio: Some(true),
        },
        notes: Vec::new(),
    })
}

pub fn convert_video(
    options: &VideoConvertOptions,
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
) -> EngineResult<Produced> {
    let input = &task.inputs[0];
    let ffmpeg = ctx
        .tools
        .ffmpeg
        .as_ref()
        .ok_or_else(|| EngineError::new(ErrorCode::ToolMissing, "FFmpeg not found"))?;
    let webm_audio = if ffmpeg.has_encoder("libopus") {
        "libopus"
    } else {
        "libvorbis"
    };
    let args = video_args(
        options.target,
        options.preset,
        options.max_height,
        options.remove_audio,
        webm_audio,
    );
    let temp = run_conversion(task, ctx, args, input_duration(&input.details))?;
    let has_audio = if options.remove_audio {
        Some(false)
    } else {
        input_has_audio(&input.details)
    };
    Ok(Produced {
        staged: Staged::File(temp),
        expectation: Expectation::Media {
            format: options.target,
            has_video: true,
            has_audio,
        },
        notes: Vec::new(),
    })
}

fn run_conversion(
    task: &TaskSpec,
    ctx: &TaskContext<'_>,
    codec_args: Vec<String>,
    duration: Option<Duration>,
) -> EngineResult<tempfile::NamedTempFile> {
    let input = &task.inputs[0];
    debug_assert!(matches!(
        input.format.map(FileFormat::family),
        Some(FormatFamily::Audio | FormatFamily::Video)
    ));
    let ffmpeg = ctx
        .tools
        .ffmpeg
        .as_ref()
        .ok_or_else(|| EngineError::new(ErrorCode::ToolMissing, "FFmpeg not found"))?;
    let temp = ctx.output.temp_file(&task.output().dir)?;
    let mut args: Vec<OsString> = [
        "-hide_banner",
        "-nostdin",
        "-y",
        "-loglevel",
        "error",
        "-progress",
        "pipe:1",
        "-nostats",
        "-i",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(ffmpeg_file_arg(&input.path));
    args.extend(codec_args.into_iter().map(OsString::from));
    args.push(ffmpeg_file_arg(temp.path()));
    run_ffmpeg(&ffmpeg.executable, args, duration, ctx)?;
    Ok(temp)
}

fn run_ffmpeg(
    ffmpeg: &Path,
    args: Vec<OsString>,
    duration: Option<Duration>,
    ctx: &TaskContext<'_>,
) -> EngineResult<()> {
    let mut child = tool_command(ffmpeg)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            EngineError::new(
                ErrorCode::ToolFailed,
                format!("could not start FFmpeg: {e}"),
            )
        })?;
    let stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let (tx, rx) = mpsc::channel::<f32>();
    let total_us = duration.map(|d| d.as_micros() as f64).filter(|t| *t > 0.0);
    let progress_thread = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let value = line
                .strip_prefix("out_time_us=")
                .or_else(|| line.strip_prefix("out_time_ms="))
                .and_then(|v| v.trim().parse::<f64>().ok());
            if let (Some(us), Some(total)) = (value, total_us) {
                let _ = tx.send((us / total).clamp(0.0, 1.0) as f32);
            }
        }
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut tail: VecDeque<u8> = VecDeque::with_capacity(STDERR_TAIL);
        let mut buf = [0u8; 1024];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            for byte in &buf[..n] {
                if tail.len() == STDERR_TAIL {
                    tail.pop_front();
                }
                tail.push_back(*byte);
            }
        }
        String::from_utf8_lossy(&tail.into_iter().collect::<Vec<_>>()).into_owned()
    });

    let timeout = ctx.limits.media_timeout(duration);
    let started = Instant::now();
    let status = loop {
        while let Ok(fraction) = rx.try_recv() {
            ctx.progress(0.02 + fraction * 0.9);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => return Err(EngineError::new(ErrorCode::ToolFailed, e.to_string())),
        }
        if ctx.cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(EngineError::cancelled());
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(EngineError::new(
                ErrorCode::Timeout,
                format!("FFmpeg ran longer than {timeout:?}"),
            ));
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    let _ = progress_thread.join();
    let stderr = stderr_thread.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        let detail = stderr.trim();
        let code = if detail.contains("No space left") {
            ErrorCode::DiskFull
        } else if detail.contains("Invalid data found") || detail.contains("could not find codec") {
            ErrorCode::Unreadable
        } else {
            ErrorCode::ToolFailed
        };
        Err(EngineError::new(
            code,
            format!("FFmpeg exited with {status}: {detail}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_probe_output() {
        let json = r#"{"streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":1080},{"codec_type":"audio","codec_name":"aac"}],"format":{"duration":"12.500"}}"#;
        let details = parse_probe(json).unwrap();
        assert_eq!(
            details,
            FileDetails::Media {
                duration_ms: Some(12_500),
                has_video: true,
                has_audio: true,
                width: Some(1920),
                height: Some(1080),
                video_codec: Some("h264".into()),
                audio_codec: Some("aac".into()),
            }
        );
    }

    #[test]
    fn cover_art_is_not_video() {
        let json = r#"{"streams":[{"codec_type":"audio","codec_name":"mp3"},{"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1}}],"format":{"duration":"3.0"}}"#;
        let FileDetails::Media {
            has_video,
            has_audio,
            ..
        } = parse_probe(json).unwrap()
        else {
            panic!()
        };
        assert!(!has_video);
        assert!(has_audio);
        assert!(parse_probe(r#"{"streams":[],"format":{}}"#).is_err());
    }

    #[test]
    fn builds_audio_arguments() {
        let args = audio_args(FileFormat::Mp3, Preset::Quality);
        assert!(args.windows(2).any(|w| w == ["-b:a", "320k"]));
        assert!(args.windows(2).any(|w| w == ["-f", "mp3"]));
        let args = audio_args(FileFormat::M4a, Preset::Smallest);
        assert!(args.windows(2).any(|w| w == ["-f", "ipod"]));
    }

    #[test]
    fn builds_video_arguments() {
        let args = video_args(
            FileFormat::Mp4,
            Preset::Balanced,
            Some(720),
            false,
            "libopus",
        );
        assert!(args.windows(2).any(|w| w == ["-crf", "23"]));
        assert!(args.iter().any(|a| a.contains("min(720")));
        assert!(args.windows(2).any(|w| w == ["-map", "0:a:0?"]));
        let args = video_args(FileFormat::Webm, Preset::Smallest, None, true, "libopus");
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.iter().any(|a| a == "libopus"));
        assert!(args.windows(2).any(|w| w == ["-f", "webm"]));
    }
}
