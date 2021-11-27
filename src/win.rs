#![allow(dead_code)]
use windows::runtime::Result;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GWLP_HWNDPARENT, GWL_EXSTYLE, GetWindowLongPtrA, LWA_ALPHA, SetLayeredWindowAttributes, SetParent, SetWindowDisplayAffinity, SetWindowLongPtrA, WDA_EXCLUDEFROMCAPTURE, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT};
use winit::platform::windows::WindowExtWindows;
use winit::window::Window;

pub(crate) fn hide_from_capture(window: &Window) -> Result<()> {
    let hwnd = window.hwnd();
    unsafe { SetWindowDisplayAffinity(HWND(hwnd as isize), WDA_EXCLUDEFROMCAPTURE) }.ok()
}

pub(crate) fn set_owner(window: &Window, owner: isize) -> isize {
    unsafe { SetWindowLongPtrA(get_hwnd(window), GWLP_HWNDPARENT, owner)}
}

pub(crate) fn set_parent(window: &Window, parent: isize) {
    unsafe { SetParent(get_hwnd(window), HWND(parent))};
}

pub(crate) fn set_transparent(window: &Window){
    let styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    let new_style = WS_EX_TRANSPARENT.0 | styles as u32;
    unsafe { SetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE, new_style as isize) };
    let new_styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    assert_eq!(new_styles ^ styles, WS_EX_TRANSPARENT.0 as isize );
}

pub(crate) fn set_layered(window: &Window){
    let styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    let new_style = WS_EX_LAYERED.0 | styles as u32;
    unsafe { SetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE, new_style as isize) };
    unsafe { SetLayeredWindowAttributes(get_hwnd(window), 0, 254, LWA_ALPHA) };
    let new_styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    assert_eq!(new_styles ^ styles, WS_EX_LAYERED.0 as isize );
}

pub(crate) fn set_noactivate(window: &Window) {
    let styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    let new_style = WS_EX_NOACTIVATE.0 | styles as u32;
    unsafe { SetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE, new_style as isize) };
    let new_styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    assert_eq!(new_styles ^ styles, WS_EX_NOACTIVATE.0 as isize );
}   

pub(crate) fn get_hwnd(window: &Window) -> HWND {
    let hwnd = window.hwnd();
    HWND(hwnd as isize)
}
