use dioxus::prelude::*;
use rfd::{AsyncFileDialog, FileHandle};

use crate::components::*;

#[component]
pub fn InputsPath(text: Signal<String>) -> Element {
    rsx! {
        div {
            input { value: "{text}", oninput: move |evt| text.set(evt.value()) }
            Button { "Primary" }

            Button { variant: ButtonVariant::Secondary, "Secondary" }

            Button { variant: ButtonVariant::Destructive, "Destructive" }

            Button { variant: ButtonVariant::Outline, "Outline" }

            Button { variant: ButtonVariant::Ghost, "Ghost" }
            Button {
                onclick: move |_| {
                    spawn(async move {
                        if let Some(files) = AsyncFileDialog::new()
                            .set_title("选择 ASS 文件")
                            .add_filter("ASS 字幕", &["ass"])
                            .pick_files()
                            .await
                        {
                            append_path(text, files);
                        }
                    });
                },
                "文件"
            }
            button {
                onclick: move |_| {
                    spawn(async move {
                        if let Some(files) = AsyncFileDialog::new()
                            .set_title("选择 ASS 目录")
                            .pick_folders()
                            .await
                        {
                            append_path(text, files);
                        }
                    });
                },
                "文件夹"
            }
        }

    }
}

#[component]
pub fn FontsPath(text: Signal<String>) -> Element {
    rsx! {
        div {
            input { value: "{text}", oninput: move |evt| text.set(evt.value()) }
            button {
                onclick: move |_| {
                    spawn(async move {
                        if let Some(files) = AsyncFileDialog::new()
                            .set_title("选择字体目录")
                            .pick_folders()
                            .await
                        {
                            append_path(text, files);
                        }
                    });
                },
                "文件夹"
            }
        }
    }
}

#[component]
pub fn OutputPath(text: Signal<String>) -> Element {
    rsx! {
        div {
            input { value: "{text}", oninput: move |evt| text.set(evt.value()) }
            button {
                onclick: move |_| {
                    spawn(async move {
                        if let Some(file) = AsyncFileDialog::new()
                            .set_title("选择输出目录")
                            .pick_folder()
                            .await
                        {
                            text.set(file.path().display().to_string());
                        }
                    });
                },
                "文件夹"
            }
        }
    }
}

#[component]
pub fn FontsIndexPath(text: Signal<String>) -> Element {
    rsx! {
        div {
            input { value: "{text}", oninput: move |evt| text.set(evt.value()) }
            button {
                onclick: move |_| {
                    spawn(async move {
                        if let Some(file) = AsyncFileDialog::new()
                            .set_title("选择字体索引目录")
                            .pick_folder()
                            .await
                        {
                            text.set(file.path().display().to_string());
                        }
                    });
                },
                "文件夹"
            }
        }
    }
}

fn append_path(mut text: Signal<String>, files: Vec<FileHandle>) {
    for file in files {
        let path = file.path().display().to_string();
        if !path.is_empty() {
            if !text.is_empty() {
                text.set(format!("{text}|{path}"));
            } else {
                text.set(path);
            }
        }
    }
}
