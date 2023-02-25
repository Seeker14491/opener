use crate::OpenError;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) fn reveal(path: &Path) -> Result<(), OpenError> {
    Command::new("explorer.exe")
        .arg("/select,")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(OpenError::Io)?;
    Ok(())
}
