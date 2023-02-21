use crate::OpenError;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    let mut open = Command::new("open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(OpenError::Io)?;

    crate::wait_child(&mut open, "open")
}

pub(crate) fn reveal(path: &Path) -> Result<(), OpenError> {
    let mut open = Command::new("open")
        .arg("-R")
        .arg("--")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(OpenError::Io)?;

    crate::wait_child(&mut open, "open")
}
