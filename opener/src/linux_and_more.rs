use crate::OpenError;
use std::ffi::OsStr;
use std::io;
use std::process::{Child, Command, Stdio};

// Generated from xdg-utils v1.2.1:
// https://gitlab.freedesktop.org/xdg/xdg-utils/-/tree/v1.2.1/scripts
const XDG_OPEN_SCRIPT: &str = include_str!("xdg-open");
// The script is passed as one argument, which Linux limits to 128 KiB.
const _: () = assert!(XDG_OPEN_SCRIPT.len() < 128 * 1024);

#[cfg(target_os = "linux")]
mod wsl;
#[cfg(target_os = "linux")]
pub(crate) use self::wsl::windows_browser_argument as wsl_to_windows_browser_argument;

#[cfg(target_os = "linux")]
pub(crate) fn open_browser(path: &OsStr) -> Result<(), OpenError> {
    if crate::is_wsl() {
        wsl::open_browser(path)
    } else {
        open(path)
    }
}

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    #[cfg(target_os = "linux")]
    if crate::is_wsl() {
        return wsl::open(path);
    }

    open_with_xdg_open(path)
}

fn open_with_xdg_open(path: &OsStr) -> Result<(), OpenError> {
    if open_with_system_xdg_open(path).is_err() {
        open_with_internal_xdg_open(path)?;
    }

    Ok(())
}

#[cfg(all(feature = "reveal", target_os = "linux"))]
pub(crate) fn reveal(path: &std::path::Path) -> Result<(), OpenError> {
    if crate::is_wsl() {
        reveal_in_windows_explorer(path)
    } else {
        crate::freedesktop::reveal_with_dbus(path).or_else(|_| reveal_fallback(path))
    }
}

#[cfg(all(feature = "reveal", not(target_os = "linux")))]
pub(crate) fn reveal(path: &std::path::Path) -> Result<(), OpenError> {
    reveal_fallback(path)
}

#[cfg(feature = "reveal")]
fn reveal_fallback(path: &std::path::Path) -> Result<(), OpenError> {
    let path = path.canonicalize().map_err(OpenError::Io)?;
    let parent = path.parent().unwrap_or(std::path::Path::new("/"));
    open(parent.as_os_str())
}

fn open_with_system_xdg_open(path: &OsStr) -> io::Result<Child> {
    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

fn open_with_internal_xdg_open(path: &OsStr) -> Result<Child, OpenError> {
    // Passing the script with -c keeps stdin free, so programs it launches get no script input.
    Command::new("sh")
        .arg("-c")
        .arg(XDG_OPEN_SCRIPT)
        .arg("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| OpenError::Spawn {
            cmds: "sh".into(),
            source: err,
        })
}

#[cfg(all(feature = "reveal", target_os = "linux"))]
fn reveal_in_windows_explorer(path: &std::path::Path) -> Result<(), OpenError> {
    let converted_path = wsl::wslpath("-w", path.as_os_str()).ok();
    let converted_path = converted_path.as_deref();
    let path = match converted_path {
        None => path,
        Some(x) => std::path::Path::new(x),
    };
    Command::new("explorer.exe")
        .arg("/select,")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| OpenError::Spawn {
            cmds: "explorer.exe".into(),
            source: err,
        })?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn is_wsl() -> bool {
    if is_docker() {
        return false;
    }

    if let Ok(true) = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|osrelease| osrelease.to_ascii_lowercase().contains("microsoft"))
    {
        return true;
    }

    if let Ok(true) = std::fs::read_to_string("/proc/version")
        .map(|version| version.to_ascii_lowercase().contains("microsoft"))
    {
        return true;
    }

    false
}

#[cfg(target_os = "linux")]
fn is_docker() -> bool {
    let has_docker_env = std::fs::metadata("/.dockerenv").is_ok();

    let has_docker_cgroup = std::fs::read_to_string("/proc/self/cgroup")
        .map(|cgroup| cgroup.to_ascii_lowercase().contains("docker"))
        .unwrap_or(false);

    has_docker_env || has_docker_cgroup
}
