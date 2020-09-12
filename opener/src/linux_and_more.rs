use crate::OpenError;
use std::{
    ffi::OsStr,
    io::{Read, Write},
    process::{Command, Stdio},
};

const XDG_OPEN_SCRIPT: &[u8] = include_bytes!("xdg-open");

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    let mut cmd = Command::new("sh")
        .args(&["-s"])
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(OpenError::Io)?;

    let child_stdin = cmd.stdin.as_mut().unwrap();
    child_stdin
        .write_all(XDG_OPEN_SCRIPT)
        .map_err(OpenError::Io)?;

    let exit_status = cmd.wait().map_err(OpenError::Io)?;
    if exit_status.success() {
        Ok(())
    } else {
        let mut stderr_output = String::new();
        cmd.stderr
            .as_mut()
            .unwrap()
            .read_to_string(&mut stderr_output)
            .ok();

        Err(OpenError::ExitStatus {
            cmd: "xdg-open (internal)",
            status: exit_status,
            stderr: stderr_output,
        })
    }
}
