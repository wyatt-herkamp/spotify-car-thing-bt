// TODO: this code should probably get swapped out with something like socket2..

use std::fmt::Write;
use crate::error;
use crate::error::windows::WindowsError;
use crate::error::AppError;
use crate::sys::{BtSocketListener, BtSocketStream, Platform};
use std::mem::size_of;
use std::mem::size_of_val;
use uuid::Uuid;
use windows::core::GUID;
use windows::core::PWSTR;
use windows::Win32::Devices::Bluetooth::*;
use windows::Win32::Networking::WinSock::*;
use windows::Win32::System::Threading::GetCurrentProcessId;

pub struct WindowsPlatform;

impl Platform for WindowsPlatform {
    type BtSocketListener = WinBtSockListener;

    fn init() -> Result<(), AppError> {
        let mut wsa_data = WSADATA::default();
        let result = unsafe { WSAStartup(0x0202, &mut wsa_data) };
        if result != 0 {
            return Err(AppError::IOError(std::io::Error::from_raw_os_error(result)));
        }
        Ok(())
    }

    fn teardown() -> Result<(), AppError> {
        WindowsError::wsa_error_if_zero(unsafe { WSACleanup() }).map_err(|e| e.into())
    }

    fn bind_bt_socket_listener() -> Result<Self::BtSocketListener, AppError> {
        let s = unsafe { socket(AF_BTH.into(), SOCK_STREAM.into(), BTHPROTO_RFCOMM as i32) };
        if s == INVALID_SOCKET {
            return Err(WindowsError::InvalidSocket.into());
        }

        let mut sa_size = size_of::<SOCKADDR_BTH>() as i32;
        let mut sa = SOCKADDR_BTH {
            addressFamily: AF_BTH,
            btAddr: 0,
            serviceClassId: GUID::default(),
            port: !0,
        };

        let res = WindowsError::wsa_error_if_zero(unsafe { bind(s, &sa as *const _ as *const SOCKADDR, sa_size)});
        if let Err(e) = res {
            close_socket(s)?;
            return Err(e.into());
        }

        let res = WindowsError::wsa_error_if_zero(unsafe { listen(s, 2) });
        if let Err(e) = res {
            close_socket(s)?;
            return Err(e.into());
        }

        let res = WindowsError::wsa_error_if_zero(unsafe { getsockname(s, &mut sa as *mut _ as *mut SOCKADDR, &mut sa_size) });
        if let Err(e) = res {
            close_socket(s)?;
            return Err(e.into());
        }

        Ok(WinBtSockListener { s, sa })
    }
}
pub struct WinBtSockListener {
    s: SOCKET,
    sa: SOCKADDR_BTH,
}
impl BtSocketListener for WinBtSockListener {
    type BtSocketStream = WinBtSockStream;

    fn register_service(&mut self, name: &'static str, uuid: Uuid) -> Result<(), AppError> {
        let mut name = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mut class_id = GUID::from_u128(uuid.as_u128());

        let mut sockinfo = CSADDR_INFO {
            LocalAddr: SOCKET_ADDRESS {
                lpSockaddr: &mut self.sa as *mut _ as *mut SOCKADDR,
                iSockaddrLength: size_of_val(&self.sa) as i32,
            },
            iSocketType: SOCK_STREAM.0.into(),
            iProtocol: size_of_val(&self.sa) as i32,
            ..CSADDR_INFO::default()
        };

        let qs = WSAQUERYSETW {
            dwSize: size_of::<WSAQUERYSETW>() as u32,
            lpszServiceInstanceName: PWSTR::from_raw(name.as_mut_ptr()),
            lpServiceClassId: &mut class_id,
            dwNameSpace: NS_BTH,
            dwNumberOfCsAddrs: 1,
            lpcsaBuffer: &mut sockinfo,
            ..WSAQUERYSETW::default()
        };

        // Windows will automatically deregister our btsdp service once our
        // process is no longer running
        //
        // We can't do it ourselves since we don't get a handle to our sdp
        // registration
        WindowsError::wsa_error_if_zero(unsafe { WSASetServiceW(&qs, RNRSERVICE_REGISTER, 0) })
            .map_err(|e| e.into())
    }

    fn accept(&mut self) -> Result<Self::BtSocketStream, AppError> {
        let mut sa = SOCKADDR_BTH::default();
        let sa_ptr = &mut sa as *mut _ as *mut SOCKADDR;
        let sa_len_ptr = &mut (size_of::<SOCKADDR_BTH>() as i32) as *mut i32;

        let s = unsafe { accept(self.s, Some(sa_ptr), Some(sa_len_ptr)) };
        WinBtSockStream::new(s, sa).map_err(|e| e.into())
    }

    fn rfcomm_port(&self) -> u32 {
        self.sa.port
    }
}

impl Drop for WinBtSockListener {
    fn drop(&mut self) {
        close_socket(self.s).expect("failed to close socket on drop")
    }
}

pub struct WinBtSockStream {
    s: SOCKET,
    sa: SOCKADDR_BTH,
}
impl BtSocketStream for WinBtSockStream{
    fn try_clone(&self) -> Result<Self, AppError> {
        let mut info = WSAPROTOCOL_INFOW::default();
        WindowsError::wsa_error_if_zero(unsafe {
            WSADuplicateSocketW(self.s, GetCurrentProcessId(), &mut info)
        })?;
        let s = unsafe {
            WSASocketW(
                info.iAddressFamily,
                info.iSocketType,
                info.iProtocol,
                Some(&info),
                0,
                0,
            )
        };

        WinBtSockStream::new(s, self.sa).map_err(|e| e.into())
    }

    fn port(&self) -> u32 {
        self.sa.port

    }
}
impl WinBtSockStream {
    pub fn new(s: SOCKET, sa: SOCKADDR_BTH) -> Result<Self, WindowsError> {
        if s == INVALID_SOCKET {
            return Err(WindowsError::InvalidSocket);
        }
        return Ok(WinBtSockStream { s, sa });
    }
}

macro_rules! impl_read_write {
    ($res:ident) => {
        match $res {
            SOCKET_ERROR => {
                let error = wsa_get_last_error();

                if error == WSAESHUTDOWN {
                    Ok(0)
                } else {
                    Err(std::io::Error::from_raw_os_error(error.0))
                }
            }
            _ => Ok($res as usize),
        }
    };
}
impl std::io::Read for WinBtSockStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let res = unsafe { recv(self.s, buf, SEND_RECV_FLAGS::default()) };
        impl_read_write!(res)
    }
}

impl std::io::Write for WinBtSockStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let res = unsafe { send(self.s, buf, SEND_RECV_FLAGS::default()) };
        impl_read_write!(res)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn close_socket(socket: SOCKET) -> error::Result<()> {
   WindowsError::wsa_error_if_zero(unsafe { closesocket(socket) }).map_err(|e| e.into())
}

fn wsa_get_last_error() -> WSA_ERROR {
    unsafe { WSAGetLastError() }
}
