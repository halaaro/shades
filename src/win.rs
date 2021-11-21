use windows::runtime::Result;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};
use winit::platform::windows::WindowExtWindows;
use winit::window::Window;

pub fn hide_from_capture(window: &Window) -> Result<()> {
    let hwnd = window.hwnd();
    unsafe { SetWindowDisplayAffinity(HWND(hwnd as isize), WDA_EXCLUDEFROMCAPTURE) }.ok()
}
