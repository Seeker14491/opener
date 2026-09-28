#![cfg_attr(docsrs, feature(doc_cfg))]

//! This crate provides the [`open`] function, which opens a file or link with the default program
//! configured on the system:
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
//! An [`open_browser`] function is also provided, for when you intend on opening a file or link in
//! a browser, specifically. This function works like the [`open`] function, but explicitly allows
//! overriding the browser launched by setting the `$BROWSER` environment variable.
//!
//! # Crate features
//!
//! - **reveal** - Enables usage of the [`reveal`] function.

#![warn(
    rust_2018_idioms,
    deprecated_in_future,
    macro_use_extern_crate,
    missing_debug_implementations,
    unused_qualifications
)]

#[cfg(any(target_os = "windows", target_os = "linux"))]
mod browser_command;
#[cfg(all(feature = "reveal", target_os = "linux"))]
mod freedesktop;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod linux_and_more;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use crate::linux_and_more as sys;
#[cfg(target_os = "macos")]
use crate::macos as sys;
#[cfg(target_os = "windows")]
use crate::windows as sys;

use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Display, Formatter};
use std::process::{Command, ExitStatus, Stdio};
use std::{env, io};

/// Opens a file or link with the system default program.
///
/// Note that a path like "rustup.rs" could potentially refer to either a file or a website. If you
/// want to open the website, you should add the "http://" prefix, for example.
///
/// Also note that a result of `Ok(())` just means a way of opening the path was found, and no error
/// occurred as a direct result of opening the path. Errors beyond that point aren't caught. For
/// example, `Ok(())` would be returned even if a file was opened with a program that can't read the
/// file, or a dead link was opened in a browser.
///
/// ## Platform Implementation Details
///
/// - On Windows, file URLs are converted to native paths before calling `ShellExecuteW`.
///   Query strings and fragments are discarded; use [`open_browser()`] to preserve them in a browser.
///   Other inputs are passed directly to `ShellExecuteW`.
/// - On Windows Subsystem for Linux (WSL), paths and file URLs are converted to Windows paths, then
///   opened with the Windows shell as on Windows, using PowerShell. If PowerShell is unavailable,
///   `xdg-open` is used as on Linux.
/// - On Mac the system `open` command is used.
/// - On Linux and other platforms, the system `xdg-open` script is used if available,
///   otherwise an `xdg-open` script embedded in this library is used.
///
/// ## Blocking
///
/// This function does not wait for the opened application to exit, but may block while preparing
/// or dispatching the launch.
/// Terminal browsers such as Lynx are not supported: the launch does not preserve interactive
/// terminal access or wait for the browser session to finish.
pub fn open<P>(path: P) -> Result<(), OpenError>
where
    P: AsRef<OsStr>,
{
    sys::open(path.as_ref())
}

/// Opens a file or link with the system default program, using the `BROWSER` environment variable
/// when set.
///
/// If the `BROWSER` environment variable is set, the program specified by it is used to open the
/// path. Otherwise, behavior is identical to [`open()`], except that on Windows and WSL, file URLs
/// receive special handling to preserve percent-encoded paths, queries, and fragments in the
/// default browser. If this is unavailable, it falls back to [`open()`], which may discard queries
/// and fragments.
///
/// ## Blocking
///
/// This function does not wait for the opened application to exit, but may block while preparing
/// or dispatching the launch.
/// Terminal browsers such as Lynx are not supported: the launch does not preserve interactive
/// terminal access or wait for the browser session to finish.
pub fn open_browser<P>(path: P) -> Result<(), OpenError>
where
    P: AsRef<OsStr>,
{
    let mut path = path.as_ref();
    if let Ok(browser_var) = env::var("BROWSER") {
        let windows_path;
        if is_wsl() && browser_var.ends_with(".exe") {
            if let Some(windows_path_2) = wsl_to_windows_browser_argument(path) {
                windows_path = windows_path_2;
                path = &windows_path;
            }
        };

        spawn_detached(
            Command::new(&browser_var)
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null()),
        )
        .map_err(|err| OpenError::Spawn {
            cmds: browser_var,
            source: err,
        })
    } else {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            sys::open_browser(path)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            sys::open(path)
        }
    }
}

/// Opens the default file explorer and reveals a file or folder in its containing folder.
///
/// ## Errors
/// This function may or may not return an error if the path does not exist.
///
/// ## Platform Implementation Details
/// - On Windows and Windows Subsystem for Linux (WSL) the `explorer.exe /select, <path>` command is used.
/// - On Mac the system `open -R` command is used.
/// - On non-WSL Linux the [`file-manager-interface`] or the [`org.freedesktop.portal.OpenURI`] DBus Interface is used if available,
///   falling back to opening the containing folder with [`open`].
/// - On other platforms, the containing folder is shown with [`open`].
///
/// [`org.freedesktop.portal.OpenURI`]: https://flatpak.github.io/xdg-desktop-portal/#gdbus-org.freedesktop.portal.OpenURI
/// [`file-manager-interface`]: https://freedesktop.org/wiki/Specifications/file-manager-interface/
#[cfg(feature = "reveal")]
pub fn reveal<P>(path: P) -> Result<(), OpenError>
where
    P: AsRef<std::path::Path>,
{
    sys::reveal(path.as_ref())
}

/// An error type representing the failure to open a path. Possibly returned by the [`open`]
/// function.
#[non_exhaustive]
#[derive(Debug)]
pub enum OpenError {
    /// An IO error occurred.
    Io(io::Error),

    /// There was an error spawning command(s).
    Spawn {
        /// The command(s) that failed to spawn.
        cmds: String,

        /// The underlying error.
        source: io::Error,
    },

    /// A command exited with a non-zero exit status.
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
            // Io is transparent: it displays the inner error, and reports that error's source.
            OpenError::Io(inner) => {
                write!(f, "{inner}")?;
            }
            OpenError::Spawn { cmds, source: _ } => {
                write!(f, "error spawning command(s) '{cmds}'")?;
            }
            OpenError::ExitStatus {
                cmd,
                status,
                stderr,
            } => {
                write!(f, "command '{cmd}' did not execute successfully; {status}")?;

                let stderr = stderr.trim();
                if !stderr.is_empty() {
                    write!(f, "\ncommand stderr:\n{stderr}")?;
                }
            }
        }

        Ok(())
    }
}

impl Error for OpenError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            OpenError::Io(inner) => inner.source(),
            OpenError::Spawn { cmds: _, source } => Some(source),
            OpenError::ExitStatus { .. } => None,
        }
    }
}

/// Spawns `command` without waiting for it to exit.
#[cfg(unix)]
fn spawn_detached(command: &mut Command) -> io::Result<()> {
    let mut child = command.spawn()?;
    // An exited child lingers as a zombie process until it is waited for, so wait in the
    // background. If no thread can be started, the child is left unreaped.
    let _ = std::thread::Builder::new()
        .name("opener-reaper".into())
        .stack_size(64 * 1024)
        .spawn(move || child.wait());
    Ok(())
}

/// Spawns `command` without waiting for it to exit.
#[cfg(not(unix))]
fn spawn_detached(command: &mut Command) -> io::Result<()> {
    command.spawn().map(drop)
}

#[cfg(target_os = "linux")]
fn is_wsl() -> bool {
    sys::is_wsl()
}

#[cfg(not(target_os = "linux"))]
fn is_wsl() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn wsl_to_windows_browser_argument(path: &OsStr) -> Option<OsString> {
    sys::wsl_to_windows_browser_argument(path)
}

#[cfg(not(target_os = "linux"))]
fn wsl_to_windows_browser_argument(_path: &OsStr) -> Option<OsString> {
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_display_the_underlying_error_once() {
        let error = OpenError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid file URL path",
        ));
        assert_eq!(error.to_string(), "invalid file URL path");
        // Error reporters print the source chain too, so the message must not appear there again.
        assert!(error.source().is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detached_children_are_reaped() {
        use std::time::Duration;

        let pid_file = env::temp_dir().join(format!("opener-reap-{}", std::process::id()));
        spawn_detached(
            Command::new("sh")
                .args(["-c", r#"echo $$ > "$0""#])
                .arg(&pid_file),
        )
        .unwrap();
        let process = (0..250)
            .find_map(|_| {
                std::thread::sleep(Duration::from_millis(20));
                let pid: u32 = std::fs::read_to_string(&pid_file)
                    .ok()?
                    .trim()
                    .parse()
                    .ok()?;
                Some(std::path::PathBuf::from(format!("/proc/{pid}")))
            })
            .unwrap();
        let _ = std::fs::remove_file(&pid_file);
        // An unreaped child stays in /proc as a zombie until this test process exits.
        assert!((0..250).any(|_| {
            std::thread::sleep(Duration::from_millis(20));
            !process.exists()
        }));
    }
}
