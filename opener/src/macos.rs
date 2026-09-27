use crate::OpenError;
use std::ffi::OsStr;
use std::process::{Command, Stdio};

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    run_open(Command::new("open").arg(path))
}

#[cfg(feature = "reveal")]
pub(crate) fn reveal(path: &std::path::Path) -> Result<(), OpenError> {
    run_open(Command::new("open").arg("-R").arg("--").arg(path))
}

fn run_open(command: &mut Command) -> Result<(), OpenError> {
    // output() drains stderr while waiting, so a full pipe cannot block the child.
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(OpenError::Io)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(OpenError::ExitStatus {
            cmd: "open",
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
