use crate::OpenError;
use std::path::Path;
use std::process::Command;

pub(crate) fn reveal(path: &Path) -> Result<(), OpenError> {
    Command::new("explorer.exe")
        .arg("/select,")
        .arg(path)
        .spawn()
        .map_err(OpenError::Io)?;
    Ok(())
}
