#![allow(dead_code)]
use std::ffi::c_void;
use std::intrinsics::transmute;
use std::mem::size_of;
use std::thread;
use std::time::Duration;

use windows::runtime::Result;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetForegroundWindow, GetWindowLongPtrA, GetWindowRect,
    SetLayeredWindowAttributes, SetParent, SetWindowDisplayAffinity, SetWindowLongPtrA,
    SetWindowsHookExW, GWLP_HWNDPARENT, GWL_EXSTYLE, LWA_ALPHA, WDA_EXCLUDEFROMCAPTURE, WH_SHELL,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::windows::WindowExtWindows;
use winit::window::Window;

pub(crate) fn hide_from_capture(window: &Window) -> Result<()> {
    let hwnd = window.hwnd();
    unsafe { SetWindowDisplayAffinity(HWND(hwnd as isize), WDA_EXCLUDEFROMCAPTURE) }.ok()
}

pub(crate) fn set_owner(window: &Window, owner: isize) -> isize {
    unsafe { SetWindowLongPtrA(get_hwnd(window), GWLP_HWNDPARENT, owner) }
}

pub(crate) fn set_parent(window: &Window, parent: isize) {
    unsafe { SetParent(get_hwnd(window), HWND(parent)) };
}

pub(crate) fn set_transparent(window: &Window) {
    let styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    let new_style = WS_EX_TRANSPARENT.0 | styles as u32;
    unsafe { SetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE, new_style as isize) };
    let new_styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    assert_eq!(new_styles ^ styles, WS_EX_TRANSPARENT.0 as isize);
}

pub(crate) fn set_layered(window: &Window) {
    let styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    let new_style = WS_EX_LAYERED.0 | styles as u32;
    unsafe { SetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE, new_style as isize) };
    unsafe { SetLayeredWindowAttributes(get_hwnd(window), 0, 254, LWA_ALPHA) };
    let new_styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    assert_eq!(new_styles ^ styles, WS_EX_LAYERED.0 as isize);
}

pub(crate) fn set_noactivate(window: &Window) {
    let styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    let new_style = WS_EX_NOACTIVATE.0 | styles as u32;
    unsafe { SetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE, new_style as isize) };
    let new_styles = unsafe { GetWindowLongPtrA(get_hwnd(window), GWL_EXSTYLE) };
    assert_eq!(new_styles ^ styles, WS_EX_NOACTIVATE.0 as isize);
}

pub(crate) fn get_hwnd(window: &Window) -> HWND {
    let hwnd = window.hwnd();
    HWND(hwnd as isize)
}

pub(crate) enum TrackEvent {
    Size(PhysicalSize<u32>),
    Position(PhysicalPosition<i32>),
}

pub(crate) fn track<F>(target: isize, callback: F)
where
    F: Fn(Option<TrackEvent>) + Send + 'static,
{
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(30));
        let mut crect: RECT = Default::default();
        let mut wrect: RECT = Default::default();
        let mut drect: RECT = Default::default();
        unsafe { GetClientRect(HWND(target), &mut crect) };
        unsafe { GetWindowRect(HWND(target), &mut wrect) };
        let res = unsafe {
            DwmGetWindowAttribute(
                HWND(target),
                DWMWA_EXTENDED_FRAME_BOUNDS,
                transmute(&mut drect),
                size_of::<RECT>() as u32,
            )
        };
        if let Err(e) = res {
            println!("Error in DwmGetWindowAttribute: {:?}", e);
            callback(None);
            break;
        }
        let size = PhysicalSize {
            width: (crect.right - crect.left) as u32,
            height: (crect.bottom - crect.top) as u32,
        };

        let pos = PhysicalPosition {
            x: drect.left + 1,
            y: drect.top + 1,
        };
        callback(Some(TrackEvent::Position(pos)));
        callback(Some(TrackEvent::Size(size)));
    });
}

pub(crate) fn get_foreground_hwnd() -> isize {
    let hwnd = unsafe { GetForegroundWindow() };
    hwnd.0
}
