use assfonts::ui;
use color_eyre::eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;
    ui::run_gui();
    Ok(())
}
