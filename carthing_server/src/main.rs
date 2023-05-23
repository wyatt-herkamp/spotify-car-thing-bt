use crate::error::AppError;
use crate::sys::{CurrentPlatform, Platform};
mod apps;
mod error;
mod sys;
mod workers;


fn main() -> Result<(), AppError> {
    simple_log::quick!();

    CurrentPlatform::init()?;



    if let Err(e) = apps::deskthing::run_deskthing::<CurrentPlatform>() {
        println!("error: {:?}", e);
    }

    CurrentPlatform::teardown()
}
