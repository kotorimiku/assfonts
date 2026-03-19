use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
};

use color_eyre::eyre::{Result, eyre};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::{
    cli::{BuildOptions, RunOptions},
    commands::{run_build, run_process},
};

#[derive(Debug, Clone)]
pub enum GuiEvent {
    Progress { percent: u8, message: String },
    Finished(TaskSummary),
    Cancelled(String),
    Failed(String),
}

#[derive(Debug)]
pub struct TaskHandle {
    pub cancel_flag: Arc<AtomicBool>,
    pub receiver: Receiver<GuiEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSummary {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BuildIndexSummary {
    total_fonts: usize,
}

#[derive(Debug, Deserialize)]
struct ProcessReportEntry {
    output_ass: Option<PathBuf>,
    missing_fonts: Vec<String>,
    missing_sample_index: Vec<usize>,
    error_index: Vec<usize>,
}

pub fn start_process_task(options: RunOptions) -> TaskHandle {
    let (sender, receiver) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(true));
    let worker_flag = Arc::clone(&cancel_flag);

    thread::spawn(move || {
        let ass_count = count_ass_inputs(&options.inputs).unwrap_or(options.inputs.len());
        let _ = sender.send(GuiEvent::Progress {
            percent: 0,
            message: format!("准备处理 {} 个 ASS 文件", ass_count),
        });

        let output_dir = options.output.clone();
        let report_requested = options.report;
        let report_path = output_dir.join("run-report.json");
        let sender_for_progress = sender.clone();

        let result = run_process(options, Arc::clone(&worker_flag), move |progress| {
            let _ = sender_for_progress.send(GuiEvent::Progress {
                percent: progress,
                message: format!("处理进度 {}%", progress),
            });
        });

        match result {
            Ok(()) => {
                let summary =
                    summarize_process(ass_count, &output_dir, report_requested, &report_path);
                let _ = sender.send(GuiEvent::Finished(summary));
            }
            Err(error) => {
                if !worker_flag.load(Ordering::Relaxed)
                    || error.to_string().contains("process was interrupted")
                {
                    let _ = sender.send(GuiEvent::Cancelled("任务已取消。".to_string()));
                } else {
                    let _ = sender.send(GuiEvent::Failed(format!("{error:#}")));
                }
            }
        }
    });

    TaskHandle {
        cancel_flag,
        receiver,
    }
}

pub fn start_build_task(options: BuildOptions) -> TaskHandle {
    let (sender, receiver) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(true));

    thread::spawn(move || {
        let _ = sender.send(GuiEvent::Progress {
            percent: 10,
            message: format!("开始扫描 {} 个字体目录", options.fontpaths.len()),
        });

        let output_dir = options.output.clone();
        let result = run_build(options);

        match result {
            Ok(()) => {
                let summary = summarize_build(&output_dir);
                let _ = sender.send(GuiEvent::Finished(summary));
            }
            Err(error) => {
                let _ = sender.send(GuiEvent::Failed(format!("{error:#}")));
            }
        }
    });

    TaskHandle {
        cancel_flag,
        receiver,
    }
}

pub fn parse_path_list(value: &str) -> Vec<PathBuf> {
    value
        .split(['\n', '|'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(PathBuf::from)
        .collect()
}

pub fn count_ass_inputs(inputs: &[PathBuf]) -> Result<usize> {
    Ok(collect_ass_inputs(inputs)?.len())
}

pub fn ensure_non_empty_paths(value: &str, field_name: &str) -> Result<Vec<PathBuf>> {
    let paths = parse_path_list(value);
    if paths.is_empty() {
        return Err(eyre!("{}不能为空", field_name));
    }
    Ok(paths)
}

fn summarize_process(
    ass_count: usize,
    output_dir: &Path,
    report_requested: bool,
    report_path: &Path,
) -> TaskSummary {
    let mut lines = vec![
        format!("处理文件数: {}", ass_count),
        format!("输出目录: {}", output_dir.display()),
    ];

    if report_requested && report_path.exists() {
        lines.push(format!("报告文件: {}", report_path.display()));

        if let Ok(bytes) = fs::read(report_path)
            && let Ok(entries) = serde_json::from_slice::<Vec<ProcessReportEntry>>(&bytes)
        {
            let completed = entries
                .iter()
                .filter(|entry| entry.output_ass.is_some())
                .count();
            let missing_samples = entries
                .iter()
                .filter(|entry| !entry.missing_sample_index.is_empty())
                .count();
            let errors = entries
                .iter()
                .filter(|entry| !entry.error_index.is_empty())
                .count();
            let mut distinct_fonts = BTreeSet::new();
            for entry in &entries {
                for font in &entry.missing_fonts {
                    distinct_fonts.insert(font.clone());
                }
            }

            lines.push(format!("成功输出: {}", completed));
            lines.push(format!("缺字样本文件: {}", missing_samples));
            lines.push(format!("覆盖错误文件: {}", errors));
            if !distinct_fonts.is_empty() {
                let preview = distinct_fonts
                    .iter()
                    .take(6)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("缺失字体: {}", preview));
            }
        }
    }

    TaskSummary {
        title: "ASS 处理完成".to_string(),
        lines,
    }
}

fn summarize_build(output_dir: &Path) -> TaskSummary {
    let index_path = output_dir.join("fonts.index.json");
    let mut lines = vec![format!("索引文件: {}", index_path.display())];

    if let Ok(bytes) = fs::read(&index_path)
        && let Ok(summary) = serde_json::from_slice::<BuildIndexSummary>(&bytes)
    {
        lines.insert(0, format!("索引字体数: {}", summary.total_fonts));
    }

    TaskSummary {
        title: "字体索引构建完成".to_string(),
        lines,
    }
}

fn collect_ass_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut ass_files = Vec::new();

    for input in inputs {
        if input.is_dir() {
            for entry in WalkDir::new(input) {
                let entry = entry?;
                if entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("ass")
                {
                    ass_files.push(entry.into_path());
                }
            }
        } else if input.is_file()
            && input.extension().and_then(|value| value.to_str()) == Some("ass")
        {
            ass_files.push(input.clone());
        }
    }

    Ok(ass_files)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::parse_path_list;

    #[test]
    fn parse_path_list_supports_newlines_and_pipes() {
        let parsed = parse_path_list("a.ass\nB:/fonts|C:/db");

        assert_eq!(
            parsed,
            vec![
                PathBuf::from("a.ass"),
                PathBuf::from("B:/fonts"),
                PathBuf::from("C:/db"),
            ]
        );
    }
}
