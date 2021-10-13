use crate::OpenError;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::{io, ptr};
use winapi::ctypes::c_int;
use winapi::um::shellapi::ShellExecuteW;

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    const SW_SHOW: c_int = 5;

    let path = convert_path(path).map_err(OpenError::Io)?;
    let operation: Vec<u16> = OsStr::new("open\0").encode_wide().collect();
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            path.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOW,
        )
    };
    if result as c_int > 32 {
        Ok(())
    } else {
        Err(OpenError::Io(io::Error::last_os_error()))
    }
}

pub(crate) fn open_in_file_manager(path: &OsStr) -> Result<(), OpenError> {
    const SW_SHOW: c_int = 5;

    let mut select_path = OsString::from("/select,\"");
    select_path.push(path);
    select_path.push('"');

    let operation: Vec<u16> = OsStr::new("open\0").encode_wide().collect();
    let explorer = convert_path("explorer.exe".as_ref()).map_err(OpenError::Io)?;
    let path = convert_path(&select_path).map_err(OpenError::Io)?;
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            explorer.as_ptr(),
            path.as_ptr(),
            ptr::null(),
            SW_SHOW,
        )
    };
    if result as c_int > 32 {
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
