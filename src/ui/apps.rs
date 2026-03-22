use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
    time::Duration,
};

use dioxus::prelude::*;
use rfd::AsyncFileDialog;

use crate::{
    cli::{BuildOptions, RunOptions},
    gui_backend::{
        GuiEvent, TaskHandle, TaskSummary, count_ass_inputs, ensure_non_empty_paths,
        parse_path_list, start_build_task, start_process_task,
    },
};

const APP_CSS: &str = include_str!("../../assets/styling/main.css");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceTab {
    Process,
    BuildIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunningTask {
    Process,
    BuildIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessFormState {
    input_paths: String,
    output_dir: String,
    font_paths: String,
    db_path: String,
    strict: bool,
    allow_missing_sample: bool,
    allow_missing_fonts: bool,
    report: bool,
    force: bool,
}

impl Default for ProcessFormState {
    fn default() -> Self {
        Self {
            input_paths: String::new(),
            output_dir: ".".to_string(),
            font_paths: String::new(),
            db_path: ".".to_string(),
            strict: true,
            allow_missing_sample: false,
            allow_missing_fonts: false,
            report: true,
            force: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildFormState {
    font_paths: String,
    output_dir: String,
}

impl Default for BuildFormState {
    fn default() -> Self {
        Self {
            font_paths: String::new(),
            output_dir: ".".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskPanelState {
    running: bool,
    current_task: Option<RunningTask>,
    progress: u8,
    status: String,
    summary: Option<TaskSummary>,
    error: Option<String>,
}

impl TaskPanelState {
    fn idle() -> Self {
        Self {
            running: false,
            current_task: None,
            progress: 0,
            status: "准备就绪。".to_string(),
            summary: None,
            error: None,
        }
    }
}

#[component]
pub fn App() -> Element {
    let mut active_tab = use_signal(|| WorkspaceTab::Process);
    let process_form = use_signal(ProcessFormState::default);
    let build_form = use_signal(BuildFormState::default);
    let task_panel = use_signal(TaskPanelState::idle);
    let receiver_signal = use_signal(|| None::<Arc<Mutex<Receiver<GuiEvent>>>>);
    let cancel_signal = use_signal(|| None::<Arc<AtomicBool>>);

    use_hook(move || {
        let mut receiver_signal = receiver_signal;
        let mut cancel_signal = cancel_signal;
        let mut task_panel = task_panel;

        spawn(async move {
            loop {
                if let Some(receiver) = receiver_signal().clone() {
                    loop {
                        let next_event = {
                            let guard = receiver.lock().expect("receiver lock poisoned");
                            guard.try_recv().ok()
                        };

                        let Some(event) = next_event else {
                            break;
                        };

                        match event {
                            GuiEvent::Progress { percent, message } => {
                                task_panel.with_mut(|panel| {
                                    panel.running = true;
                                    panel.progress = percent;
                                    panel.status = message;
                                });
                            }
                            GuiEvent::Finished(summary) => {
                                task_panel.with_mut(|panel| {
                                    panel.running = false;
                                    panel.progress = 100;
                                    panel.status = summary.title.clone();
                                    panel.summary = Some(summary);
                                    panel.error = None;
                                });
                                receiver_signal.set(None);
                                cancel_signal.set(None);
                            }
                            GuiEvent::Cancelled(message) => {
                                task_panel.with_mut(|panel| {
                                    panel.running = false;
                                    panel.status = message;
                                    panel.progress = 0;
                                    panel.summary = None;
                                    panel.error = None;
                                });
                                receiver_signal.set(None);
                                cancel_signal.set(None);
                            }
                            GuiEvent::Failed(message) => {
                                task_panel.with_mut(|panel| {
                                    panel.running = false;
                                    panel.status = "任务执行失败。".to_string();
                                    panel.progress = 0;
                                    panel.summary = None;
                                    panel.error = Some(message);
                                });
                                receiver_signal.set(None);
                                cancel_signal.set(None);
                            }
                        }
                    }
                }

                tokio::time::sleep(Duration::from_millis(80)).await;
            }
        });
    });

    let is_running = task_panel().running;

    let start_process = {
        let process_form = process_form;
        let mut task_panel = task_panel;
        let mut receiver_signal = receiver_signal;
        let mut cancel_signal = cancel_signal;
        let mut active_tab = active_tab;

        move |_| {
            if task_panel().running {
                return;
            }

            let form = process_form();
            match build_run_options(&form) {
                Ok(options) => {
                    let handle = start_process_task(options);
                    apply_task_handle(
                        handle,
                        RunningTask::Process,
                        "正在启动 ASS 处理任务...",
                        &mut task_panel,
                        &mut receiver_signal,
                        &mut cancel_signal,
                    );
                    active_tab.set(WorkspaceTab::Process);
                }
                Err(error) => {
                    task_panel.with_mut(|panel| {
                        panel.running = false;
                        panel.current_task = Some(RunningTask::Process);
                        panel.status = "表单校验失败。".to_string();
                        panel.summary = None;
                        panel.error = Some(format!("{error:#}"));
                    });
                }
            }
        }
    };

    let start_build = {
        let build_form = build_form;
        let mut task_panel = task_panel;
        let mut receiver_signal = receiver_signal;
        let mut cancel_signal = cancel_signal;
        let mut active_tab = active_tab;

        move |_| {
            if task_panel().running {
                return;
            }

            let form = build_form();
            match build_build_options(&form) {
                Ok(options) => {
                    let handle = start_build_task(options);
                    apply_task_handle(
                        handle,
                        RunningTask::BuildIndex,
                        "正在构建字体索引...",
                        &mut task_panel,
                        &mut receiver_signal,
                        &mut cancel_signal,
                    );
                    active_tab.set(WorkspaceTab::BuildIndex);
                }
                Err(error) => {
                    task_panel.with_mut(|panel| {
                        panel.running = false;
                        panel.current_task = Some(RunningTask::BuildIndex);
                        panel.status = "表单校验失败。".to_string();
                        panel.summary = None;
                        panel.error = Some(format!("{error:#}"));
                    });
                }
            }
        }
    };

    let cancel_task = {
        let cancel_signal = cancel_signal;
        let mut task_panel = task_panel;

        move |_| {
            if let Some(flag) = cancel_signal().clone() {
                flag.store(false, Ordering::Relaxed);
                task_panel.with_mut(|panel| {
                    panel.status = "正在请求取消任务...".to_string();
                });
            }
        }
    };

    rsx! {
        document::Title { "assfonts GUI" }
        style { {APP_CSS} }

        div { class: "app-shell",
            header { class: "hero-header",
                div { class: "hero-copy",
                    p { class: "eyebrow", "ASS 字幕字体工作台" }
                    h1 { "assfonts 桌面图形界面" }
                    p { class: "hero-subtitle",
                        "在同一个窗口里完成 ASS 字幕处理和字体索引构建，复用现有 CLI 核心逻辑。"
                    }
                }
                div { class: "hero-meta",
                    span { class: "meta-chip", "Dioxus Desktop" }
                    span { class: "meta-chip", "并行处理" }
                    span { class: "meta-chip", "可取消任务" }
                }
            }

            div { class: "tab-strip",
                button {
                    class: if active_tab() == WorkspaceTab::Process { "tab-button active" } else { "tab-button" },
                    disabled: is_running && task_panel().current_task == Some(RunningTask::BuildIndex),
                    onclick: move |_| active_tab.set(WorkspaceTab::Process),
                    "处理 ASS"
                }
                button {
                    class: if active_tab() == WorkspaceTab::BuildIndex { "tab-button active" } else { "tab-button" },
                    disabled: is_running && task_panel().current_task == Some(RunningTask::Process),
                    onclick: move |_| active_tab.set(WorkspaceTab::BuildIndex),
                    "构建索引"
                }
            }

            div { class: "workspace-grid",
                section { class: "panel form-panel",
                    if active_tab() == WorkspaceTab::Process {
                        ProcessWorkspace {
                            form: process_form,
                            disabled: is_running,
                            on_start: start_process,
                        }
                    } else {
                        BuildWorkspace {
                            form: build_form,
                            disabled: is_running,
                            on_start: start_build,
                        }
                    }
                }

                section { class: "panel status-panel",
                    TaskSummaryPanel { state: task_panel(), on_cancel: cancel_task }
                }
            }
        }
    }
}

#[component]
fn ProcessWorkspace(
    form: Signal<ProcessFormState>,
    disabled: bool,
    on_start: EventHandler<MouseEvent>,
) -> Element {
    let state = form();

    let choose_input_files = {
        let mut form = form;
        move |_| {
            spawn(async move {
                if let Some(paths) = AsyncFileDialog::new()
                    .set_title("选择 ASS 文件")
                    .add_filter("ASS 字幕", &["ass"])
                    .pick_files()
                    .await
                {
                    let paths = paths
                        .into_iter()
                        .map(|path| path.path().to_path_buf())
                        .collect::<Vec<_>>();

                    form.with_mut(|current| {
                        current.input_paths = merge_paths(&current.input_paths, &paths);
                    });
                }
            });
        }
    };

    let choose_input_dirs = {
        let mut form = form;
        move |_| {
            spawn(async move {
                if let Some(paths) = AsyncFileDialog::new()
                    .set_title("选择包含 ASS 的目录")
                    .pick_folders()
                    .await
                {
                    let paths = paths
                        .into_iter()
                        .map(|path| path.path().to_path_buf())
                        .collect::<Vec<_>>();

                    form.with_mut(|current| {
                        current.input_paths = merge_paths(&current.input_paths, &paths);
                    });
                }
            });
        }
    };

    let choose_fonts = {
        let mut form = form;
        move |_| {
            spawn(async move {
                if let Some(paths) = AsyncFileDialog::new()
                    .set_title("选择字体目录")
                    .pick_folders()
                    .await
                {
                    let paths = paths
                        .into_iter()
                        .map(|path| path.path().to_path_buf())
                        .collect::<Vec<_>>();

                    form.with_mut(|current| {
                        current.font_paths = merge_paths(&current.font_paths, &paths);
                    });
                }
            });
        }
    };

    let choose_output = {
        let mut form = form;
        move |_| {
            spawn(async move {
                if let Some(path) = AsyncFileDialog::new()
                    .set_title("选择输出目录")
                    .pick_folder()
                    .await
                {
                    form.with_mut(|current| {
                        current.output_dir = path.path().display().to_string();
                    });
                }
            });
        }
    };

    let choose_db = {
        let mut form = form;
        move |_| {
            spawn(async move {
                if let Some(path) = AsyncFileDialog::new()
                    .set_title("选择索引目录")
                    .pick_folder()
                    .await
                {
                    form.with_mut(|current| {
                        current.db_path = path.path().display().to_string();
                    });
                }
            });
        }
    };

    rsx! {
        div { class: "panel-header",
            h2 { "处理 ASS" }
            p { "选择字幕文件、字体来源和输出目录，执行嵌入与子集化。" }
        }

        FieldBlock {
            label: "输入 ASS 文件或目录",
            helper: "支持多行输入，也可用右侧按钮批量添加。",
            value: state.input_paths.clone(),
            disabled,
            multiline: true,
            on_change: move |value| form.with_mut(|current| current.input_paths = value),
            actions: rsx! {
                button {
                    class: "action-button secondary",
                    disabled,
                    onclick: choose_input_files,
                    "添加文件"
                }
                button {
                    class: "action-button secondary",
                    disabled,
                    onclick: choose_input_dirs,
                    "添加目录"
                }
            },
        }

        FieldBlock {
            label: "字体目录",
            helper: "可选。留空时会尝试从索引目录中的 fonts.index.json 加载。",
            value: state.font_paths.clone(),
            disabled,
            multiline: true,
            on_change: move |value| form.with_mut(|current| current.font_paths = value),
            actions: rsx! {
                button { class: "action-button secondary", disabled, onclick: choose_fonts, "选择字体目录" }
            },
        }

        FieldBlock {
            label: "输出目录",
            helper: "处理后的 ASS 文件和报告会写入这里。",
            value: state.output_dir.clone(),
            disabled,
            multiline: false,
            on_change: move |value| form.with_mut(|current| current.output_dir = value),
            actions: rsx! {
                button { class: "action-button secondary", disabled, onclick: choose_output, "浏览" }
            },
        }

        FieldBlock {
            label: "索引目录",
            helper: "该目录下若存在 fonts.index.json，会作为字体来源补充。",
            value: state.db_path.clone(),
            disabled,
            multiline: false,
            on_change: move |value| form.with_mut(|current| current.db_path = value),
            actions: rsx! {
                button { class: "action-button secondary", disabled, onclick: choose_db, "浏览" }
            },
        }

        div { class: "options-grid",
            ToggleOption {
                label: "严格模式",
                description: "遇到字体问题时立即失败。",
                selected: state.strict,
                disabled,
                on_toggle: move |_| form.with_mut(|current| current.strict = !current.strict),
            }
            ToggleOption {
                label: "允许缺字样本",
                description: "允许部分字符样本缺失。",
                selected: state.allow_missing_sample,
                disabled,
                on_toggle: move |_| {
                    form
                        .with_mut(|current| {
                        current.allow_missing_sample = !current.allow_missing_sample;
                    })
                },
            }
            ToggleOption {
                label: "允许缺失字体",
                description: "字体未匹配到时继续执行。",
                selected: state.allow_missing_fonts,
                disabled,
                on_toggle: move |_| {
                    form
                        .with_mut(|current| {
                        current.allow_missing_fonts = !current.allow_missing_fonts;
                    })
                },
            }
            ToggleOption {
                label: "生成报告",
                description: "输出 run-report.json 摘要文件。",
                selected: state.report,
                disabled,
                on_toggle: move |_| form.with_mut(|current| current.report = !current.report),
            }
            ToggleOption {
                label: "覆盖输出",
                description: "允许覆盖现有输出文件。",
                selected: state.force,
                disabled,
                on_toggle: move |_| form.with_mut(|current| current.force = !current.force),
            }
        }

        div { class: "footer-actions",
            button {
                class: "action-button primary",
                disabled,
                onclick: on_start,
                "开始处理"
            }
        }
    }
}

#[component]
fn BuildWorkspace(
    form: Signal<BuildFormState>,
    disabled: bool,
    on_start: EventHandler<MouseEvent>,
) -> Element {
    let state = form();

    let choose_fonts = {
        let mut form = form;
        move |_| {
            spawn(async move {
                if let Some(paths) = AsyncFileDialog::new()
                    .set_title("选择字体目录")
                    .pick_folders()
                    .await
                {
                    let paths = paths
                        .into_iter()
                        .map(|path| path.path().to_path_buf())
                        .collect::<Vec<_>>();

                    form.with_mut(|current| {
                        current.font_paths = merge_paths(&current.font_paths, &paths);
                    });
                }
            });
        }
    };

    let choose_output = {
        let mut form = form;
        move |_| {
            spawn(async move {
                if let Some(path) = AsyncFileDialog::new()
                    .set_title("选择输出目录")
                    .pick_folder()
                    .await
                {
                    form.with_mut(|current| {
                        current.output_dir = path.path().display().to_string();
                    });
                }
            });
        }
    };

    rsx! {
        div { class: "panel-header",
            h2 { "构建字体索引" }
            p { "扫描多个字体目录，生成可复用的 fonts.index.json。" }
        }

        FieldBlock {
            label: "字体目录",
            helper: "支持多行输入。每一行一个目录。",
            value: state.font_paths.clone(),
            disabled,
            multiline: true,
            on_change: move |value| form.with_mut(|current| current.font_paths = value),
            actions: rsx! {
                button { class: "action-button secondary", disabled, onclick: choose_fonts, "选择目录" }
            },
        }

        FieldBlock {
            label: "输出目录",
            helper: "索引文件将命名为 fonts.index.json。",
            value: state.output_dir.clone(),
            disabled,
            multiline: false,
            on_change: move |value| form.with_mut(|current| current.output_dir = value),
            actions: rsx! {
                button { class: "action-button secondary", disabled, onclick: choose_output, "浏览" }
            },
        }

        div { class: "footer-actions",
            button {
                class: "action-button primary",
                disabled,
                onclick: on_start,
                "开始建库"
            }
        }
    }
}

#[component]
fn FieldBlock(
    label: &'static str,
    helper: &'static str,
    value: String,
    disabled: bool,
    multiline: bool,
    on_change: EventHandler<String>,
    actions: Element,
) -> Element {
    rsx! {
        div { class: "field-block",
            div { class: "field-copy",
                label { class: "field-label", "{label}" }
                p { class: "field-helper", "{helper}" }
            }
            div { class: "field-body",
                if multiline {
                    textarea {
                        class: "text-input multiline",
                        disabled,
                        value: value.clone(),
                        oninput: move |event| on_change.call(event.value()),
                    }
                } else {
                    input {
                        class: "text-input",
                        disabled,
                        value: value.clone(),
                        oninput: move |event| on_change.call(event.value()),
                    }
                }
                div { class: "field-actions", {actions} }
            }
        }
    }
}

#[component]
fn ToggleOption(
    label: &'static str,
    description: &'static str,
    selected: bool,
    disabled: bool,
    on_toggle: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if selected { "toggle-card selected" } else { "toggle-card" },
            disabled,
            onclick: on_toggle,
            div { class: "toggle-title-row",
                span { class: "toggle-title", "{label}" }
                span { class: "toggle-state",
                    if selected {
                        "开启"
                    } else {
                        "关闭"
                    }
                }
            }
            p { class: "toggle-description", "{description}" }
        }
    }
}

#[component]
fn TaskSummaryPanel(state: TaskPanelState, on_cancel: EventHandler<MouseEvent>) -> Element {
    let progress_style = format!("width: {}%;", state.progress);
    let task_name = match state.current_task {
        Some(RunningTask::Process) => "ASS 处理任务",
        Some(RunningTask::BuildIndex) => "字体索引任务",
        None => "尚未运行任务",
    };

    rsx! {
        div { class: "panel-header",
            h2 { "任务状态" }
            p { "展示进度、取消控制和运行结果摘要。" }
        }

        div { class: "status-card",
            div { class: "status-topline",
                span { class: "status-task-name", "{task_name}" }
                span { class: if state.running { "status-badge running" } else { "status-badge" },
                    if state.running {
                        "运行中"
                    } else {
                        "空闲"
                    }
                }
            }
            p { class: "status-message", "{state.status}" }
            div { class: "progress-shell",
                div { class: "progress-fill", style: progress_style }
            }
            div { class: "progress-meta",
                span { "完成度" }
                span { "{state.progress}%" }
            }
            if state.running && state.current_task == Some(RunningTask::Process) {
                button { class: "action-button danger", onclick: on_cancel, "取消任务" }
            }
        }

        if let Some(summary) = state.summary.clone() {
            div { class: "result-card",
                h3 { "{summary.title}" }
                ul { class: "summary-list",
                    for line in summary.lines {
                        li { "{line}" }
                    }
                }
            }
        }

        if let Some(error) = state.error.clone() {
            div { class: "result-card error-card",
                h3 { "错误详情" }
                pre { class: "error-output", "{error}" }
            }
        }
    }
}

fn apply_task_handle(
    handle: TaskHandle,
    task: RunningTask,
    status: &str,
    task_panel: &mut Signal<TaskPanelState>,
    receiver_signal: &mut Signal<Option<Arc<Mutex<Receiver<GuiEvent>>>>>,
    cancel_signal: &mut Signal<Option<Arc<AtomicBool>>>,
) {
    task_panel.with_mut(|panel| {
        panel.running = true;
        panel.current_task = Some(task);
        panel.progress = 0;
        panel.status = status.to_string();
        panel.summary = None;
        panel.error = None;
    });
    receiver_signal.set(Some(Arc::new(Mutex::new(handle.receiver))));
    cancel_signal.set(Some(handle.cancel_flag));
}

fn build_run_options(form: &ProcessFormState) -> color_eyre::eyre::Result<RunOptions> {
    let inputs = ensure_non_empty_paths(&form.input_paths, "输入路径")?;
    let input_count = count_ass_inputs(&inputs)?;
    if input_count == 0 {
        return Err(color_eyre::eyre::eyre!("未找到可处理的 ASS 文件"));
    }

    let fontpaths = {
        let paths = parse_path_list(&form.font_paths);
        if paths.is_empty() { None } else { Some(paths) }
    };

    Ok(RunOptions {
        inputs,
        output: if form.output_dir.trim().is_empty() {
            ".".into()
        } else {
            form.output_dir.trim().into()
        },
        fontpaths,
        dbpath: if form.db_path.trim().is_empty() {
            ".".into()
        } else {
            form.db_path.trim().into()
        },
        strict: form.strict,
        allow_missing_sample: form.allow_missing_sample,
        allow_missing_fonts: form.allow_missing_fonts,
        report: form.report,
        force: form.force,
    })
}

fn build_build_options(form: &BuildFormState) -> color_eyre::eyre::Result<BuildOptions> {
    Ok(BuildOptions {
        fontpaths: ensure_non_empty_paths(&form.font_paths, "字体目录")?,
        output: if form.output_dir.trim().is_empty() {
            ".".into()
        } else {
            form.output_dir.trim().into()
        },
    })
}

fn merge_paths(existing: &str, new_paths: &[std::path::PathBuf]) -> String {
    let mut merged = parse_path_list(existing)
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();

    for path in new_paths {
        let rendered = path.display().to_string();
        if !merged.contains(&rendered) {
            merged.push(rendered);
        }
    }

    merged.join("\n")
}
