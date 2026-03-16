fn main() {
    #[cfg(feature = "gui")]
    slint_build::compile("ui/app-window.slint").expect("Slint build failed");
}
