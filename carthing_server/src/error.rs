use std::fmt::{Display, Formatter};
use std::net::TcpStream;
use crossbeam_channel::SendError;
use thiserror::Error;
use tungstenite::{HandshakeError, ServerHandshake};
use tungstenite::handshake::server::NoCallback;

pub type Result<T> = std::result::Result<T, AppError>;
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Windows(windows::WindowsError),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Unable to Send a message to a channel")]
    SendError,
    #[error(transparent)]
    WebSocketHandshakeError(#[from] HandshakeError<ServerHandshake<TcpStream, NoCallback>>)
}
impl<T> From<SendError<T>> for AppError{
    fn from(_: SendError<T>) -> Self {
        AppError::SendError
    }
}
#[cfg(windows)]
pub mod windows {
    use std::error::Error;
    use crate::error::AppError;
    use std::fmt::{Display, Formatter};

    #[derive(Debug)]
    pub enum WindowsError {
        WSAError(windows::Win32::Networking::WinSock::WSA_ERROR),
        InvalidSocket,
    }
    impl Error for WindowsError{}
    impl From<WindowsError> for AppError {
        fn from(e: WindowsError) -> Self {
            AppError::Windows(e)
        }
    }

    impl Display for WindowsError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                WindowsError::WSAError(e) => {
                    write!(f, "WSAError: {:?}", std::io::Error::from_raw_os_error(e.0))
                }
                v => write!(f, "WindowsError: {:?}", v),
            }
        }
    }
    impl WindowsError {
        pub fn wsa_error_if_zero(res: i32) -> Result<(), WindowsError> {
            if res == 0 {
                Ok(())
            } else {
                Err(WindowsError::WSAError(unsafe {
                    windows::Win32::Networking::WinSock::WSAGetLastError()
                }))
            }
        }

    }
}
