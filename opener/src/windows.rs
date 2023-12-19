use crate::OpenError;
use normpath::PathExt;
use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

#[cfg(feature = "reveal")]
mod reveal;
#[cfg(feature = "reveal")]
pub(crate) use self::reveal::reveal;

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    let Err(first_error) = open_helper(path) else {
        return Ok(());
    };

    match PathBuf::from(path).normalize() {
        Ok(normalized) => match open_helper(normalized.as_os_str()) {
            Ok(()) => Ok(()),
            Err(_second_error) => Err(first_error),
        },
        Err(_) => Err(first_error),
    }
}

pub(crate) fn open_helper(path: &OsStr) -> Result<(), OpenError> {
    let path = convert_path(path).map_err(OpenError::Io)?;
    let result = unsafe {
        ShellExecuteW(
            HWND(0),
            w!("open"),
            PCWSTR::from_raw(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOW,
        )
    };
    if result.0 > 32 {
        Ok(())
    } else {
        Err(OpenError::Io(io::Error::last_os_error()))
    }
}

fn convert_path(path: &OsStr) -> io::Result<Vec<u16>> {
    let mut maybe_result: Vec<u16> = path.encode_wide().collect();
    if maybe_result.iter().any(|&u| u == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL byte(s)",
        ));
    }

    maybe_result.push(0);
    Ok(maybe_result)
}
