#![doc(html_root_url = "https://docs.rs/opener/0.4.1")]

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

#![warn(
    rust_2018_idioms,
    deprecated_in_future,
    macro_use_extern_crate,
    missing_debug_implementations,
    unused_qualifications
)]

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod linux_and_more;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use crate::linux_and_more::open as open_sys;
#[cfg(target_os = "macos")]
use crate::macos::open as open_sys;
#[cfg(target_os = "windows")]
use crate::windows::open as open_sys;

use std::{
    error::Error,
    ffi::OsStr,
    fmt::{self, Display, Formatter},
    io,
    process::ExitStatus,
};

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

/// An error type representing the failure to open a path. Possibly returned by the [`open`]
/// function.
///
/// The `ExitStatus` variant will never be returned on Windows.
#[derive(Debug)]
pub enum OpenError {
    /// An IO error occurred.
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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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

impl Error for OpenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            OpenError::Io(inner) => Some(inner),
            OpenError::ExitStatus { .. } => None,
        }
    }
}
