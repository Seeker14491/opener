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
///
/// The `ExitStatus` variant will never be returned on Windows.
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

        /// Anything the process wrote to stderr.
        stderr: String,
    },
}

impl Display for OpenError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            OpenError::Io(_) => {
                write!(f, "IO error")?;
            }
            OpenError::ExitStatus {
                cmd,
                status,
                stderr,
            } => {
                write!(
                    f,
                    "command '{}' did not execute successfully; {}",
                    cmd, status
                )?;

                let stderr = stderr.trim();
                if !stderr.is_empty() {
                    write!(f, "\ncommand stderr:\n{}", stderr)?;
                }
            }
        }

        Ok(())
    }
}

impl From<io::Error> for OpenError {
    fn from(err: io::Error) -> Self {
        OpenError::Io(err)
    }
}

/// Opens a file or link with the system default program.
///
/// Note that a path like "rustup.rs" could potentially refer to either a file or a website. If you
/// want to open the website, you should add the "http://" prefix, for example.
///
/// Also note that a result of `Ok(())` just means a way of opening the path was found, and no error
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
fn open_sys(path: &OsStr) -> Result<(), OpenError> {
    open_not_windows("open", path, &[], None, "open")
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_sys(path: &OsStr) -> Result<(), OpenError> {
    const XDG_OPEN_SCRIPT: &[u8] = include_bytes!("xdg-open");

    open_not_windows(
        "sh",
        path,
        &["-s"],
        Some(XDG_OPEN_SCRIPT),
        "xdg-open (internal)",
    )
}

#[cfg(not(target_os = "windows"))]
fn open_not_windows(
    cmd: &str,
    path: &OsStr,
    extra_args: &[&str],
    piped_input: Option<&[u8]>,
    cmd_friendly_name: &'static str,
) -> Result<(), OpenError> {
    use std::{
        io::{Read, Write},
        process::{Command, Stdio},
    };

    let stdin_type = if piped_input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };

    let mut cmd = Command::new(cmd)
        .args(extra_args)
        .arg(path)
        .stdin(stdin_type)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = cmd.stdin.as_mut() {
        stdin.write_all(piped_input.unwrap())?;
    }

    let exit_status = cmd.wait()?;
    if exit_status.success() {
        Ok(())
    } else {
        let mut stderr = String::new();
        cmd.stderr.as_mut().unwrap().read_to_string(&mut stderr)?;

        Err(OpenError::ExitStatus {
            cmd: cmd_friendly_name,
            status: exit_status,
            stderr,
        })
    }
}
