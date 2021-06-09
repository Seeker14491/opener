use crate::OpenError;
use std::ffi::OsStr;
use std::io;
use std::io::Write;
use std::process::{Child, Command, Stdio};

const XDG_OPEN_SCRIPT: &[u8] = include_bytes!("xdg-open");

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    if crate::is_wsl() {
        wsl_open(path)
    } else {
        non_wsl_open(path)
    }
}

fn wsl_open(path: &OsStr) -> Result<(), OpenError> {
    let result = open_with_wslview(path);
    if let Ok(mut child) = result {
        return crate::wait_child(&mut child, "wslview".into());
    }

    let mut system_xdg_open = open_with_system_xdg_open(path).map_err(OpenError::Io)?;
    crate::wait_child(&mut system_xdg_open, "xdg-open (system)".into())
}

fn non_wsl_open(path: &OsStr) -> Result<(), OpenError> {
    let system_xdg_open = open_with_system_xdg_open(path);

    let system_xdg_open_used;
    let mut xdg_open;
    match system_xdg_open {
        Ok(child) => {
            system_xdg_open_used = true;
            xdg_open = child;
        }
        Err(_) => {
            system_xdg_open_used = false;
            xdg_open = open_with_internal_xdg_open(path)?;
        }
    };

    let cmd_name = if system_xdg_open_used {
        "xdg-open (system)"
    } else {
        "xdg-open (internal)"
    };

    crate::wait_child(&mut xdg_open, cmd_name.into())
}

fn open_with_wslview(path: &OsStr) -> io::Result<Child> {
    let converted_path = crate::wsl_to_windows_path(path);
    let converted_path = converted_path.as_deref();
    let path = match converted_path {
        None => path,
        Some(x) => x,
    };

    Command::new("wslview")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
}

fn open_with_system_xdg_open(path: &OsStr) -> io::Result<Child> {
    Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
}

fn open_with_internal_xdg_open(path: &OsStr) -> Result<Child, OpenError> {
    let mut sh = Command::new("sh")
        .arg("-s")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(OpenError::Io)?;

    sh.stdin
        .as_mut()
        .unwrap()
        .write_all(XDG_OPEN_SCRIPT)
        .map_err(OpenError::Io)?;

    Ok(sh)
}
