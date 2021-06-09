use crate::OpenError;
use std::ffi::OsStr;
use std::io::Write;
use std::process::{Command, Stdio};

const XDG_OPEN_SCRIPT: &[u8] = include_bytes!("xdg-open");

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    if crate::is_wsl() {
        wsl_open(path)
    } else {
        non_wsl_open(path)
    }
}

fn wsl_open(path: &OsStr) -> Result<(), OpenError> {
    let transformed_path = crate::wsl_to_windows_path(path);
    let transformed_path = transformed_path.as_deref();
    let path = match transformed_path {
        None => path,
        Some(x) => x,
    };
    let wslview = Command::new("wslview")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();

    if let Ok(mut child) = wslview {
        return crate::wait_child(&mut child, "wslview".into());
    }

    let mut system_xdg_open = Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(OpenError::Io)?;

    crate::wait_child(&mut system_xdg_open, "xdg-open (system)".into())
}

fn non_wsl_open(path: &OsStr) -> Result<(), OpenError> {
    let system_xdg_open = Command::new("xdg-open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();

    let system_xdg_open_used;
    let mut xdg_open = match system_xdg_open {
        Ok(child) => {
            system_xdg_open_used = true;
            child
        }
        Err(_) => {
            system_xdg_open_used = false;
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

            sh
        }
    };

    let cmd_name = if system_xdg_open_used {
        "xdg-open (system)"
    } else {
        "xdg-open (internal)"
    };

    crate::wait_child(&mut xdg_open, cmd_name.into())
}
