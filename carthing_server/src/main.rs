use anyhow::Result;

mod apps;
mod sys;
mod workers;

fn main() -> Result<()> {
    simple_log::quick!();
    sys::platform_init()?;

    if let Err(e) = apps::deskthing::run_deskthing() {
        println!("error: {:?}", e);
    }

    sys::platform_teardown()?;
    Ok(())
}
