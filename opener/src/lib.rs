//! This crate provides the [`open`] function, which opens a file or link with the default program
//! configured on the system.
//!
//! ```no_run
//! # fn main() -> Result<(), ::opener::OpenError> {
//! // open a website
//! opener::open("https://www.rust-lang.org")?;
//!
//! // open a file
//! opener::open("../Cargo.toml")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Platform Implementation Details
//! On Windows the `ShellExecuteW` Windows API function is used. On Mac the system `open` command is
//! used. On other platforms, the `xdg-open` script is used. The system `xdg-open` is not used;
//! instead a version is embedded within this library.
extern crate failure;
#[macro_use]
extern crate failure_derive;

#[cfg(target_os = "windows")]
extern crate winapi;

#[cfg(target_os = "windows")]
use windows::open_sys;

use std::{
    ffi::OsStr,
    fmt::{self, Display, Formatter},
    io,
    process::ExitStatus,
};

/// An error type representing the failure to open a path. Possibly returned by the [`open`]
/// function.
#[derive(Debug, Fail)]
pub enum OpenError {
    /// An IO error occurred.
    #[cause]
    Io(io::Error),

    /// The command exited with a non-zero exit status.
    ExitStatus {
        /// A string that identifies the command.
        cmd: &'static str,

        /// The failed process's exit status.
        status: ExitStatus,
    },
}

impl Display for OpenError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            OpenError::Io(_) => write!(f, "IO error"),
            OpenError::ExitStatus { cmd, status } => write!(
                f,
                "command '{}' did not execute successfully; {}",
                cmd, status
            ),
        }
    }
}

impl From<io::Error> for OpenError {
    fn from(err: io::Error) -> Self {
        OpenError::Io(err)
    }
}

/// Opens a file or link with the system default program.
///
/// Note that a result of `Ok(())` just means a way of opening the path was found, and no error
/// occurred as a direct result of opening the path. Errors beyond that point aren't caught. For
/// example, `Ok(())` would be returned even if a file was opened with a program that can't read the
/// file, or a dead link was opened in a browser.
pub fn open<P>(path: P) -> Result<(), OpenError>
where
    P: AsRef<OsStr>,
{
    open_sys(path.as_ref())
}

#[cfg(target_os = "windows")]
mod windows {
    use super::OpenError;
    use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, ptr};
    use winapi::{ctypes::c_int, um::shellapi::ShellExecuteW};

    pub fn open_sys(path: &OsStr) -> Result<(), OpenError> {
        const SW_SHOW: c_int = 5;

        let path = convert_path(path)?;
        let operation: Vec<u16> = OsStr::new("open\0").encode_wide().collect();
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation.as_ptr(),
                path.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOW,
            )
        };
        if result as c_int > 32 {
            Ok(())
        } else {
            Err(io::Error::last_os_error().into())
        }
    }

    fn convert_path(path: &OsStr) -> io::Result<Vec<u16>> {
        let mut maybe_result: Vec<u16> = path.encode_wide().collect();
        if maybe_result.iter().any(|&u| u == 0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains NUL byte(s)",
            ));
        }
        maybe_result.push(0);
        Ok(maybe_result)
    }
}

#[cfg(target_os = "macos")]
pub fn open_sys(path: &OsStr) -> Result<(), OpenError> {
    use std::process::Command;

    let exit_status = Command::new("open").arg(path).status()?;
    if exit_status.success() {
        Ok(())
    } else {
        Err(OpenError::ExitStatus {
            cmd: "open",
            status: exit_status,
        })
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn open_sys(path: &OsStr) -> Result<(), OpenError> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    const XDG_OPEN_SCRIPT: &[u8] = include_bytes!("xdg-open");

    let mut sh = Command::new("sh")
        .arg("-s")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    {
        let stdin = sh.stdin.as_mut().unwrap();
        stdin.write_all(XDG_OPEN_SCRIPT)?;
    }

    let exit_status = sh.wait()?;
    if exit_status.success() {
        Ok(())
    } else {
        Err(OpenError::ExitStatus {
            cmd: "xdg-open (internal)",
            status: exit_status,
        })
    }
}
