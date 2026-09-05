use crate::OpenError;
use normpath::PathExt;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::{io, ptr};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW;

mod browser;
pub(crate) use self::browser::open_browser;

#[cfg(feature = "reveal")]
mod reveal;
#[cfg(feature = "reveal")]
pub(crate) use self::reveal::reveal;

pub(crate) fn open(path: &OsStr) -> Result<(), OpenError> {
    let converted_path = file_url_to_path(path).ok().flatten();
    let path = converted_path.as_deref().unwrap_or(path);

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
    if result as usize as isize > 32 {
        Ok(())
    } else {
        Err(OpenError::Io(io::Error::last_os_error()))
    }
}

fn convert_path(path: &OsStr) -> io::Result<Vec<u16>> {
    let mut maybe_result: Vec<u16> = path.encode_wide().collect();
    if maybe_result.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL byte(s)",
        ));
    }

    maybe_result.push(0);
    Ok(maybe_result)
}

fn file_url_to_path(url: &OsStr) -> io::Result<Option<OsString>> {
    if !starts_with_ascii_case_insensitive(url, "file:") {
        return Ok(None);
    }

    let url = url.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "file URL is not valid Unicode")
    })?;
    let url =
        url::Url::parse(url).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let path = url.to_file_path().map_err(|()| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "file URL cannot be converted to a Windows path",
        )
    })?;
    Ok(Some(path.into_os_string()))
}

fn starts_with_ascii_case_insensitive(value: &OsStr, prefix: &str) -> bool {
    let mut value = value.encode_wide();
    prefix.bytes().all(|expected| {
        value.next().is_some_and(|actual| {
            actual <= u16::from(u8::MAX) && (actual as u8).eq_ignore_ascii_case(&expected)
        })
    })
}

fn hresult_error(function: &str, result: i32) -> io::Error {
    io::Error::other(format!(
        "{function} failed with HRESULT 0x{:08X}",
        result as u32
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_percent_encoded_file_url_to_path() {
        let url = OsStr::new("file:///C:/Users/John/Desktop/Me%C5%82/index.html#fragment");
        let path = file_url_to_path(url).unwrap().unwrap();

        assert_eq!(
            path,
            OsString::from(r"C:\Users\John\Desktop\Meł\index.html")
        );
    }

    #[test]
    fn distinguishes_escaped_path_characters_from_query_and_fragment() {
        let url = OsStr::new("file:///C:/dir/name%23part%20two.html?query#fragment");
        let path = file_url_to_path(url).unwrap().unwrap();

        assert_eq!(path, OsString::from(r"C:\dir\name#part two.html"));
    }

    #[test]
    fn converts_unc_file_url_to_path() {
        let url = OsStr::new("file://server/share/Me%C5%82/index.html");
        let path = file_url_to_path(url).unwrap().unwrap();

        assert_eq!(path, OsString::from(r"\\server\share\Meł\index.html"));
    }

    #[test]
    fn does_not_decode_percent_escapes_in_native_paths_or_http_urls() {
        assert_eq!(
            file_url_to_path(OsStr::new(r"C:\Users\John\Desktop\Me%C5%82\index.html")).unwrap(),
            None
        );
        assert_eq!(
            file_url_to_path(OsStr::new("https://example.com/Me%C5%82#fragment")).unwrap(),
            None
        );
    }
}
