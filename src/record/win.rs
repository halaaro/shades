use screenshot::{
    create_capture_item_for_monitor, create_d3d_device, create_direct3d_device,
    get_d3d_interface_from_object,
};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use windows::core::{ComInterface, IInspectable, Result};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::{
    Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem},
    DirectX::DirectXPixelFormat,
};
use windows::Win32::Graphics::{
    Direct3D11::{
        ID3D11Resource, ID3D11Texture2D, D3D11_BIND_FLAG, D3D11_CPU_ACCESS_READ,
        D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_RESOURCE_MISC_FLAG, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_STAGING,
    },
    Gdi::{MonitorFromWindow, MONITOR_DEFAULTTOPRIMARY},
};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

pub struct ScreenRecorder {
    item_size: windows::Graphics::SizeInt32,
    frame_pool: Direct3D11CaptureFramePool,
    session: windows::Graphics::Capture::GraphicsCaptureSession,
    receiver: std::sync::mpsc::Receiver<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D>,
    d3d_context: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    frame_count: Arc<Mutex<usize>>,
    data: Arc<Mutex<Vec<u8>>>,
}

impl ScreenRecorder {
    pub fn new(item: GraphicsCaptureItem) -> Result<Self> {
        let item_size = item.Size()?;

        let d3d_device = create_d3d_device()?;
        let d3d_context = unsafe { d3d_device.GetImmediateContext()? };
        let device = create_direct3d_device(&d3d_device)?;
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            item_size,
        )?;
        let session = frame_pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsCursorCaptureEnabled(false);

        let frame_count = Arc::new(Mutex::new(0));

        let (sender, receiver) = channel();
        frame_pool.FrameArrived(
            &TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new({
                let d3d_device = d3d_device.clone();
                let d3d_context = d3d_context.clone();
                let frame_count = frame_count.clone();
                move |frame_pool, _| unsafe {
                    let frame_pool = frame_pool.as_ref().unwrap();
                    let frame = frame_pool.TryGetNextFrame()?;

                    {
                        let mut frame_count = frame_count.lock().unwrap();
                        if *frame_count > 1 {
                            return Ok(());
                        }
                        *frame_count += 1;
                    }

                    let source_texture: ID3D11Texture2D =
                        get_d3d_interface_from_object(&frame.Surface()?)?;
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    source_texture.GetDesc(&mut desc);
                    desc.BindFlags = D3D11_BIND_FLAG(0);
                    desc.MiscFlags = D3D11_RESOURCE_MISC_FLAG(0);
                    desc.Usage = D3D11_USAGE_STAGING;
                    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
                    let copy_texture = {
                        let mut texture = None;
                        d3d_device.CreateTexture2D(&desc, None, Some(&mut texture))?;
                        texture.unwrap()
                    };

                    d3d_context
                        .CopyResource(Some(&copy_texture.cast()?), Some(&source_texture.cast()?));
                    sender.send(copy_texture).unwrap();

                    Ok(())
                }
            }),
        )?;

        session.StartCapture()?;

        let data = Arc::new(Mutex::new(vec![]));

        Ok(ScreenRecorder {
            item_size,
            frame_pool,
            session,
            receiver,
            d3d_context,
            frame_count,
            data,
        })
    }

    pub fn capture_primary() -> Result<Self> {
        let monitor_handle =
            unsafe { MonitorFromWindow(GetDesktopWindow(), MONITOR_DEFAULTTOPRIMARY) };
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
        }

        let screenshot = unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc as *mut _);

            let resource: ID3D11Resource = texture.cast()?;
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.d3d_context.Map(
                Some(&resource.clone()),
                0,
                D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )?;

            let slice: &[u8] = {
                std::slice::from_raw_parts(
                    mapped.pData as *const _,
                    (desc.Height * mapped.RowPitch) as usize,
                )
            };

            let bytes_per_pixel = 4;
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

            self.d3d_context.Unmap(Some(&resource), 0);

            Screenshot {
                data: Arc::clone(&self.data),
                height: self.item_size.Height as u32,
                width: self.item_size.Width as u32,
            }
        };

        Ok(screenshot)
    }
}

