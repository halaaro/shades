use std::{error::Error, intrinsics::transmute};

use wgpu_hal::{Api, Device, dx12::Texture};
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};
use windows::{
    runtime::Interface,
    Win32::Graphics::Direct3D11::{ID3D11Resource, ID3D11Texture2D, D3D11_TEXTURE2D_DESC},
};
// use wgu;

type DynResult = Result<(), Box<dyn Error>>;

fn main() -> DynResult {
    screenshot::init()?;

    let rec = screenshot::ScreenRecorder::capture_primary()?;
    let native_texture = rec.next_texture_raw();
    println!("{:?}", native_texture);

    let wgpu_texture: Texture = ScreenshotTexture(native_texture).into();
    println!("are we wgpu yet? {:?}", wgpu_texture);
    Ok(())
}

pub struct ScreenshotTexture(ID3D11Texture2D);

impl From<ScreenshotTexture> for wgpu_hal::dx12::Texture {
    fn from(texture: ScreenshotTexture) -> Self {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.0.GetDesc(&mut desc) };
        // println!("{:?}", desc);

        let resource: ID3D11Resource = texture.0.cast().unwrap();
        // println!("{:?}", resource);

        // FIXME: WRONG??!!!
        let resource2 = unsafe { transmute(resource) };

        // println!("{:?}", resource2);
        unsafe {
            wgpu_hal::dx12::Device::texture_from_raw(
                resource2,
                TextureFormat::Bgra8Unorm,
                TextureDimension::D2,
                Extent3d{ width: desc.Width, height: desc.Height, depth_or_array_layers: 1},
                1,
                1,
            )
        }
    }
}
