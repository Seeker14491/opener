use crate::OpenError;
use std::{
    ffi::OsStr,
    io::Read,
    process::{Command, Stdio},
};

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    let mut cmd = Command::new("open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
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
            cmd: "open",
            status: exit_status,
            stderr: stderr_output,
        })
    }
}
