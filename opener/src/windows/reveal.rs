use super::convert_path;
use crate::OpenError;
use normpath::PathExt;
use std::path::Path;
use std::{io, ptr, thread};
use winapi::shared::minwindef::{DWORD, UINT};
use winapi::shared::ntdef::PCWSTR;
use winapi::shared::winerror::HRESULT;
use winapi::um::combaseapi::{CoInitializeEx, CoUninitialize};
use winapi::um::objbase::COINIT_MULTITHREADED;
use winapi::um::shtypes::{
    PCIDLIST_ABSOLUTE, PCUITEMID_CHILD_ARRAY, PIDLIST_ABSOLUTE, PIDLIST_RELATIVE,
};

pub(crate) fn reveal(path: &Path) -> Result<(), OpenError> {
    let path = path.to_owned();
    thread::Builder::new()
        .spawn(move || reveal_in_thread(&path).map_err(OpenError::Io))
        .map_err(OpenError::Io)?
        .join()
        .expect("COM worker thread should not panic")
}

fn reveal_in_thread(path: &Path) -> io::Result<()> {
    to_io_result(unsafe { CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED) })?;
    let item_id_list = ItemIdList::new(path)?;
    // Because the cidl argument is zero, SHOpenFolderAndSelectItems opens the singular item
    // in our item id list in its containing folder and selects it.
    to_io_result(unsafe { SHOpenFolderAndSelectItems(item_id_list.0, 0, ptr::null(), 0) })?;
    unsafe { CoUninitialize() };
    Ok(())
}

fn to_io_result(hresult: HRESULT) -> io::Result<()> {
    if hresult >= 0 {
        Ok(())
    } else {
        // See the HRESULT_CODE macro in winerror.h
        Err(io::Error::from_raw_os_error(hresult & 0xFFFF))
    }
}

struct ItemIdList(PIDLIST_ABSOLUTE);

impl ItemIdList {
    fn new(path: &Path) -> io::Result<Self> {
        // The ILCreateFromPathW function expects a canonicalized path.
        // Unfortunately it does not support NT UNC paths (which std::path::canonicalize returns)
        // so we use the normpath crate instead.
        let path = convert_path(path.normalize()?.as_os_str())?;
        let result = unsafe { ILCreateFromPathW(path.as_ptr()) };
        if result.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(ItemIdList(result))
        }
    }
}

impl Drop for ItemIdList {
    fn drop(&mut self) {
        unsafe { ILFree(self.0) }
    }
}

#[link(name = "Shell32")]
extern "C" {
    fn ILCreateFromPathW(pszPath: PCWSTR) -> PIDLIST_ABSOLUTE;

    fn ILFree(pidl: PIDLIST_RELATIVE);

    fn SHOpenFolderAndSelectItems(
        pidlFolder: PCIDLIST_ABSOLUTE,
        cidl: UINT,
        apidl: PCUITEMID_CHILD_ARRAY,
        dwFlags: DWORD,
    ) -> HRESULT;
}
