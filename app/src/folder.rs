//! The system folder dialog, so an export can go wherever the author wants it,
//! plus a way to open that folder afterwards. Windows only; elsewhere Folio
//! simply asks for the path by hand.

use std::path::{Path, PathBuf};

#[cfg(windows)]
pub fn pick_folder(title: &str, start: Option<&Path>) -> Option<PathBuf> {
    use std::ffi::c_void;

    const BIF_RETURNONLYFSDIRS: u32 = 0x0001;
    const BIF_USENEWUI: u32 = 0x0050;
    const BFFM_INITIALIZED: u32 = 1;
    const BFFM_SETSELECTIONW: u32 = 0x0467;
    const COINIT_APARTMENTTHREADED: u32 = 0x2;

    #[repr(C)]
    struct BrowseInfo {
        hwnd_owner: isize,
        pidl_root: *const c_void,
        display_name: *mut u16,
        title: *const u16,
        flags: u32,
        callback: Option<unsafe extern "system" fn(isize, u32, isize, isize) -> i32>,
        callback_data: isize,
        image: i32,
    }

    unsafe extern "system" fn select_start(
        hwnd: isize,
        message: u32,
        _param: isize,
        data: isize,
    ) -> i32 {
        if message == BFFM_INITIALIZED && data != 0 {
            SendMessageW(hwnd, BFFM_SETSELECTIONW, 1, data);
        }
        0
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHBrowseForFolderW(info: *mut BrowseInfo) -> *mut c_void;
        fn SHGetPathFromIDListW(pidl: *const c_void, path: *mut u16) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetActiveWindow() -> isize;
        fn SendMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *const c_void, mode: u32) -> i32;
        fn CoUninitialize();
        fn CoTaskMemFree(memory: *const c_void);
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let title = wide(title);
    let start_wide = start.map(|path| wide(&path.to_string_lossy()));
    let mut display = [0u16; 260];
    let mut info = BrowseInfo {
        hwnd_owner: unsafe { GetActiveWindow() },
        pidl_root: std::ptr::null(),
        display_name: display.as_mut_ptr(),
        title: title.as_ptr(),
        flags: BIF_RETURNONLYFSDIRS | BIF_USENEWUI,
        callback: Some(select_start),
        callback_data: start_wide.as_ref().map_or(0, |path| path.as_ptr() as isize),
        image: 0,
    };

    unsafe {
        // The newer dialog style wants COM on this thread.
        let com = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED);
        let pidl = SHBrowseForFolderW(&mut info);
        let chosen = if pidl.is_null() {
            None
        } else {
            let mut path = [0u16; 1024];
            let ok = SHGetPathFromIDListW(pidl, path.as_mut_ptr());
            CoTaskMemFree(pidl);
            if ok == 0 {
                None
            } else {
                let len = path.iter().position(|&unit| unit == 0).unwrap_or(0);
                let text = String::from_utf16_lossy(&path[..len]);
                if text.trim().is_empty() {
                    None
                } else {
                    Some(PathBuf::from(text))
                }
            }
        };
        if com >= 0 {
            CoUninitialize();
        }
        chosen
    }
}

#[cfg(not(windows))]
pub fn pick_folder(_title: &str, _start: Option<&Path>) -> Option<PathBuf> {
    None
}

/// Show a folder in the system's file manager.
pub fn open_folder(path: &Path) {
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(not(windows))]
    let opener = "xdg-open";
    let _ = std::process::Command::new(opener).arg(path).spawn();
}
