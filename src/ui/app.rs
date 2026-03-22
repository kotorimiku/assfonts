use dioxus::prelude::*;

use crate::ui::inputs::{FontsIndexPath, FontsPath, InputsPath, OutputPath};

const APP_CSS: Asset = asset!("../../assets/dx-components-theme.css");

#[component]
pub fn App() -> Element {
    let inputs = use_signal(|| "".to_string());
    let fonts = use_signal(|| "".to_string());
    let output = use_signal(|| "".to_string());
    let fonts_index_dir = use_signal(|| "".to_string());
    rsx! {
        document::Stylesheet { href: APP_CSS }
        div {
            InputsPath { text: inputs }
            FontsPath { text: fonts }
            OutputPath { text: output }
            FontsIndexPath { text: fonts_index_dir }
        }
    }
}
