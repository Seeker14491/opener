use super::convert_path;
use crate::OpenError;
use normpath::PathExt;
use std::path::Path;
use std::{io, thread};
use windows::core::PCWSTR;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};

pub(crate) fn reveal(path: &Path) -> Result<(), OpenError> {
    let path = path.to_owned();
    thread::Builder::new()
        .spawn(move || reveal_in_thread(&path).map_err(OpenError::Io))
        .map_err(OpenError::Io)?
        .join()
        .expect("COM worker thread should not panic")
}

fn reveal_in_thread(path: &Path) -> io::Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)?;
        let result = create_and_open_item_list(path);
        CoUninitialize();

        result
    }
}

fn create_and_open_item_list(path: &Path) -> io::Result<()> {
    // The ILCreateFromPathW function expects a canonicalized path.
    // Unfortunately it does not support NT UNC paths (which std::path::canonicalize returns)
    // so we use the normpath crate instead.
    let path = convert_path(path.normalize()?.as_os_str())?;

    unsafe {
        let item_id_list = ILCreateFromPathW(PCWSTR::from_raw(path.as_ptr()));
        let result = SHOpenFolderAndSelectItems(item_id_list, None, 0);
        ILFree(Some(item_id_list));

        Ok(result?)
    }
}
