#[cfg(windows)]
mod win;

use crate::error::AppError;
use std::io::{Read,Write};
use uuid::Uuid;
pub trait BtSocketStream: Read + Write +Send {

    fn try_clone(&self) -> Result<Self, AppError> where Self: Sized;

    fn port(&self) -> u32;
}

pub trait BtSocketListener {
    type BtSocketStream: BtSocketStream;
    fn register_service(&mut self, name: &'static str, uuid: Uuid) -> Result<(), AppError>;

    fn accept(&mut self) -> Result<Self::BtSocketStream, AppError>;

    fn rfcomm_port(&self) -> u32;
}

/// Platform specific implementation
///
/// Does not have context. Just a collection of static methods
pub trait Platform {
    type BtSocketListener: BtSocketListener;
    /// Initialize the platform. This will return zero context
    fn init() -> Result<(), AppError>;
    /// Teardown the platform. This will return zero context
    fn teardown() -> Result<(), AppError>;

    fn bind_bt_socket_listener() -> Result<Self::BtSocketListener, AppError>;
}

#[cfg(windows)]
pub type CurrentPlatform = win::WindowsPlatform;
#[cfg(not(windows))]
compile_error!("Unsupported platform");