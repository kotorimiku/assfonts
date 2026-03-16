use assfonts::gui;
use color_eyre::eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;
    gui::run_gui()
}
