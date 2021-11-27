mod win;

use std::{
    cmp::{max, min},
    collections::hash_map::DefaultHasher,
    default::Default,
    hash::Hasher,
    sync::{mpsc::TrySendError, Arc},
    time::Duration,
};

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use pixels::{Error, Pixels, SurfaceTexture};

pub fn main() -> Result<(), Error> {
    // screenshot::init();

    let show_decoration = std::env::var("SHADES_NO_WIN_DECORATION").as_deref() != Ok("1");
    let always_on_top = std::env::var("SHADES_NO_ALWAYS_ON_TOP").as_deref() != Ok("1");
    let perf_mode = std::env::var("SHADES_PERF_MODE").as_deref() == Ok("1");
    let parent_win = std::env::var("SHADES_PARENT_WIN").ok().and_then(|s| s.parse::<isize>().ok());


    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Shades")
        .with_visible(false)
        .with_decorations(show_decoration)
        .with_always_on_top(always_on_top)
        .build(&event_loop)
        .expect("Could not build window");

    let id = window.id();

    win::hide_from_capture(&window).expect("could not hide window from capture");
    
    if let Some(parent) = parent_win {
        // win::set_child(&window);
        win::set_parent(&window, parent);
    }
        
        let (pix_sender, pix_receiver) = std::sync::mpsc::sync_channel(1);
        // TODO: create capture thread
        let window = Arc::new(window);
        let winref = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            let recorder =
            screenshot::ScreenRecorder::capture_primary().expect("could not capture primary");
            
            let mut last_hash = 0;
            let mut hasher: DefaultHasher = Default::default();
        loop {
            let pix = recorder.next().expect("could  not take screenshot");
            hasher.write(&pix.data.lock().unwrap());
            let hash = hasher.finish();
            if hash != last_hash {
                winref.request_redraw();
                match pix_sender.try_send(pix) {
                    Err(TrySendError::Disconnected(_)) => break,
                    Err(TrySendError::Full(_)) => (),
                    Ok(_) => (),
                };

                if !perf_mode {
                    std::thread::sleep(Duration::from_millis(30));
                }
                hasher = Default::default();
                last_hash = hash;

                //
            }
        }
    });

    // let pix = screenshot::capture().expect("could not get screenshot");
    // let mut pix = screenshot::capture_win(&window).expect("could not get screenshot");
    let mut pix = pix_receiver.recv().unwrap();
    println!("found {} x {} pixels", pix.width, pix.height);

    let mut pixels = {
        let window_size = window.as_ref().inner_size();
        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());
        Pixels::new(window_size.width, window_size.height, surface_texture)?
    };

    // var pos = window.inner_position().unwrap();
    window.request_redraw();

    window.set_visible(true);
    // win::set_transparent(&window);
    // win::set_layered(&window);

    let mut cnt = 0;
    let mut dir = 1;

    let mut last_hash = 0;
    let mut hasher: DefaultHasher = Default::default();
    let mut frame_num = 0;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        frame_num += 1;

        if frame_num == 2 {
            let size = window.inner_size();
            pixels.resize_surface(size.width, size.height);
            pixels.resize_buffer(size.width, size.height);
            window.request_redraw();
        }

        if let Event::RedrawRequested(_) = event {
            let pix = &mut pix;
            let pix_opt = {
                // drain channel
                let mut tmp = None;
                loop {
                    // drain
                    let trypix = pix_receiver.try_recv();
                    if let Ok(p) = trypix {
                        *pix = p;
                        tmp = Some(&pix)
                    // }
                    } else {
                        break;
                    }
                }
                tmp
            };
            if let Some(pix) = pix_opt {
                // let offset_x = 0;
                let target_width = window.inner_size().width as usize;
                let target_height = window.inner_size().width as usize;
                if target_width == 0 || target_height == 0 {
                    return;
                }
                // let src_width = pix.width;
                let src_height = pix.width as i32;
                let mut offset_x = 0;
                let mut offset_y = 0;
                if let Ok(pos) = window.inner_position() {
                    offset_x = pos.x;
                    offset_y = pos.y;
                }
                let max_j = (pix.width * pix.height - 1) as i32;
                // println!("found {} x {} pixels", pix.width, pix.height);
                {
                    let data = pix.data.lock().unwrap();
                    for (i, pixel) in pixels.get_frame().chunks_exact_mut(4).enumerate() {
                        let col = (i % target_height) as i32;
                        let row = (i / target_height) as i32;
                        let j = max(
                            0,
                            min(max_j, (row + offset_y) * src_height + col + offset_x),
                        ) as usize;
                        pixel[2] = 255 - data[j * 4];
                        pixel[1] = 255 - data[j * 4 + 1];
                        pixel[0] = 255 - data[j * 4 + 2];
                        hasher.write(pixel);
                        // pixel[0] = 255-pix.data[j*4];
                        // pixel[1] = 255-pix.data[j*4+1];
                        // pixel[2] = 255-pix.data[j*4+2];
                    }
                }
                let frame = pixels.get_frame();
                let flash = (cnt % 16) << 4;
                for j in 0..10 {
                    for i in 0..10 {
                        // TODO: ensure small windows do not cause panic
                        let k = (i + j * target_width as usize) * 4;
                        frame[k] ^= flash;
                        // frame[k+1] = flash;
                        // frame[k+2] = flash;
                    }
                }
                if dir == 1 {
                    cnt += 1;
                } else {
                    cnt -= 1;
                }
                if cnt == 0 || cnt == 15 {
                    dir = -dir
                };
                let hash = hasher.finish();
                if last_hash != hash
                    && pixels
                        .render()
                        .map_err(|e| panic!("pixels.render() failed: {:?}", e))
                        .is_err()
                {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                last_hash = hash;
                hasher = Default::default();
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == id => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if window_id == id && size.width > 0 && size.height > 0 => {
                println!("resized to {:?}!", size);

                pixels.resize_surface(size.width, size.height);
                pixels.resize_buffer(size.width, size.height);
                window.request_redraw();
            }
            _ => (),
        }

        // window.request_redraw();
    });

}

// mod picker {
//     use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
//     use windows::{runtime::*, Graphics::Capture::*, Win32::Foundation::*, Win32::UI::Shell::*};

//     pub fn pick_picker<W: HasRawWindowHandle>(window: &W) -> Result<GraphicsCaptureItem> {
//         let hwnd = match window.raw_window_handle() {
//             RawWindowHandle::Windows(handle) => {
//                 assert!(!handle.hwnd.is_null());
//                 HWND(handle.hwnd as isize)
//             }
//             _ => panic!("Unsupported platform"),
//         };

//         let picker = GraphicsCapturePicker::new()?;
//         let iw: IInitializeWithWindow = picker.cast()?;
//         // SAFETY:
//         // 1. hwnd is valid window handle and not null
//         // 2. picker is valid
//         unsafe { iw.Initialize(hwnd) }?;
//         println!("Waiting for selection...");
//         let result = picker.PickSingleItemAsync()?.get()?;
//         println!("{:?}", result);

//         Ok(result)
//     }
// }
