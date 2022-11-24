#![allow(dead_code)]
#![allow(unused_attributes)]
#![allow(unused_imports)]

mod capture;
mod d3d;
mod display_info;
mod window_info;

use std::borrow::Borrow;
use std::sync::Mutex;
use std::sync::Arc;
use core::sync::atomic::AtomicUsize;
use windows::Graphics::Capture::GraphicsCaptureAccess;
use windows::Graphics::Capture::GraphicsCaptureAccessKind;
use windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext1;
use windows::runtime::{IInspectable, Interface, Result};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapEncoder, BitmapPixelFormat};
use windows::Storage::{CreationCollisionOption, FileAccessMode, StorageFolder};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Resource, ID3D11Texture2D, D3D11_BIND_FLAG, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ,
    D3D11_RESOURCE_MISC_FLAG, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, HMONITOR, MONITOR_DEFAULTTOPRIMARY};
use windows::Win32::System::WinRT::{
    IGraphicsCaptureItemInterop, RoInitialize, RO_INIT_MULTITHREADED,
};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

// use capture::enumerate_capturable_windows;
// use clap::{value_t, App, Arg};
// use display_info::enumerate_displays;
// use std::io::Write;
use std::sync::mpsc::channel;
// use window_info::WindowInfo;

use raw_window_handle as raw;

fn create_capture_item_for_window(window_handle: HWND) -> Result<GraphicsCaptureItem> {
    let interop = windows::runtime::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    unsafe { interop.CreateForWindow(window_handle) }
}

fn create_capture_item_for_monitor(monitor_handle: HMONITOR) -> Result<GraphicsCaptureItem> {
    let interop = windows::runtime::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    unsafe { interop.CreateForMonitor(monitor_handle) }
}

pub fn init() -> Result<()> {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    Ok(())
}

#[derive(Default)]
pub struct Screenshot {
    pub data: Arc<Mutex<Vec<u8>>>,
    pub width: u32,
    pub height: u32,
}



// pub fn capture_win(window: &dyn raw::HasRawWindowHandle) -> Result<Screenshot> {
//     let hwnd = match window.raw_window_handle() {
//         raw::RawWindowHandle::Windows(handle) => HWND(handle.hwnd as isize),
//         _ => panic!("os not supported"),
//     };

//     let item = create_capture_item_for_window(hwnd)?;

//     let pixels = take_screenshot(&item)?;

//     Ok(pixels)
// }

pub struct ScreenRecorder {
    item_size: windows::Graphics::SizeInt32,
    frame_pool: Direct3D11CaptureFramePool,
    session: windows::Graphics::Capture::GraphicsCaptureSession,
    receiver: std::sync::mpsc::Receiver<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D>,
    d3d_context: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    frame_count: Arc<Mutex<usize>>,
    data: Arc<Mutex<Vec<u8>>>
}

impl ScreenRecorder {
    pub fn new(item: GraphicsCaptureItem) -> Result<Self> {
        let item_size = item.Size()?;

        let d3d_device = d3d::create_d3d_device()?;
        let d3d_context = unsafe {
            let mut d3d_context = None;
            d3d_device.GetImmediateContext(&mut d3d_context);
            d3d_context.unwrap()
        };
        let device = d3d::create_direct3d_device(&d3d_device)?;
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            &item_size,
        )?;
        let session = frame_pool.CreateCaptureSession(item)?;
        // GraphicsCaptureAccess::(GraphicsCaptureAccessKind::Borderless)?;
        // session.SetIsBorderRequired(false)?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        
        let frame_count = Arc::new(Mutex::new(0));

        let (sender, receiver) = channel();
        frame_pool.FrameArrived(
            TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new({
                let d3d_device = d3d_device.clone();
                let d3d_context = d3d_context.clone();
                // let session = session.clone();
                let frame_count = frame_count.clone();
                move |frame_pool, _| unsafe {
                    // if  { return Ok(()) }
                    

                    let frame_pool = frame_pool.as_ref().unwrap();
                    let frame = frame_pool.TryGetNextFrame()?;

                    {
                        let mut frame_count = frame_count.lock().unwrap();
                        if  *frame_count > 1 {
                            // std::thread::sleep(std::time::Duration::from_millis(10));
                            return Ok(())
                        }
                        *frame_count += 1;

                        // println!("frame arrived -> frame_count = {}", *frame_count);
                    }

                    let source_texture: ID3D11Texture2D =
                        d3d::get_d3d_interface_from_object(&frame.Surface()?)?;
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    source_texture.GetDesc(&mut desc);
                    desc.BindFlags = D3D11_BIND_FLAG(0);
                    desc.MiscFlags = D3D11_RESOURCE_MISC_FLAG(0);
                    desc.Usage = D3D11_USAGE_STAGING;
                    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
                    let copy_texture = { d3d_device.CreateTexture2D(&desc, std::ptr::null())? };

                    d3d_context
                        .CopyResource(Some(copy_texture.cast()?), Some(source_texture.cast()?));

                    // session.Close()?;
                    // frame_pool.Close()?;

                    sender.send(copy_texture).unwrap();

                    Ok(())
                }
            }),
        )?;

        session.StartCapture()?;

        let data = Arc::new(Mutex::new(vec![]));

        Ok(ScreenRecorder {
            // TODO: add enum with type of recorder (eg. primary, screen#, win + handle, etc.)
            item_size,
            frame_pool,
            session,
            receiver,
            d3d_context,
            frame_count,
            data
        })
    }

    pub fn capture_primary() -> Result<Self> {
        // let start_usage = get_mem_usage();
    
        let monitor_handle = unsafe { MonitorFromWindow(GetDesktopWindow(), MONITOR_DEFAULTTOPRIMARY) };
        let item = create_capture_item_for_monitor(monitor_handle)?;
        
        Self::new(item)
    }

    pub fn next_texture_raw(&self) -> ID3D11Texture2D {
        self.receiver.recv().unwrap()
    }

    pub fn next(&self) -> Result<Screenshot> {
        let texture = self.receiver.recv().unwrap();
        {
            let mut frame_count = self.frame_count.lock().unwrap();
            *frame_count -= 1;
            // println!("next -> frame_count = {}", *frame_count);
        }

        let screenshot = unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc as *mut _);

            let resource: ID3D11Resource = texture.cast()?;
            let mapped = self
                .d3d_context
                .Map(Some(resource.clone()), 0, D3D11_MAP_READ, 0)?;

            // Get a slice of bytes
            let slice: &[u8] = {
                std::slice::from_raw_parts(
                    mapped.pData as *const _,
                    (desc.Height * mapped.RowPitch) as usize,
                )
            };

            let bytes_per_pixel = 4;
            //let mut bits = vec![0u8; (desc.Width * desc.Height * bytes_per_pixel) as usize];
            let mut lock = self.data.lock().expect("poison mutex");
            let data: &mut Vec<u8> = lock.as_mut();
            let size = (desc.Width * desc.Height * bytes_per_pixel) as usize;
            data.resize(size, 0);
            for row in 0..desc.Height {
                let data_begin = (row * (desc.Width * bytes_per_pixel)) as usize;
                let data_end = ((row + 1) * (desc.Width * bytes_per_pixel)) as usize;
                let slice_begin = (row * mapped.RowPitch) as usize;
                let slice_end = slice_begin + (desc.Width * bytes_per_pixel) as usize;
                data[data_begin..data_end].copy_from_slice(&slice[slice_begin..slice_end]);
            }

            self.d3d_context.Unmap(Some(resource), 0);

            Screenshot {
                data: Arc::clone(&self.data),
                height: self.item_size.Height as u32,
                width: self.item_size.Width as u32,
            }
        };

        Ok(screenshot)
    }
}

impl Drop for ScreenRecorder {
    fn drop(&mut self) {
        self.frame_pool.Close().unwrap();
        self.session.Close().unwrap();
    }
}

fn get_mem_usage() -> usize {
    use windows::Win32::System::{ProcessStatus, Threading};
    let handle = unsafe { Threading::GetCurrentProcess() };
    let mut counters = ProcessStatus::PROCESS_MEMORY_COUNTERS::default();
    unsafe {
        ProcessStatus::K32GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of_val(&counters) as u32,
        );
    }
    counters.WorkingSetSize
}
