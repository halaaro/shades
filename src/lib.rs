mod cache;
mod win;

use std::{
    cmp::{max, min},
    collections::hash_map::DefaultHasher,
    default::Default,
    hash::Hasher,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::TrySendError,
        Arc,
    },
    time::Duration,
};

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use pixels::{Error, Pixels, SurfaceTexture};

pub fn main() -> Result<(), Error> {
    let show_decoration = std::env::var("SHADES_NO_WIN_DECORATION").as_deref() != Ok("1");
    let always_on_top = std::env::var("SHADES_NO_ALWAYS_ON_TOP").as_deref() != Ok("1");
    let perf_mode = std::env::var("SHADES_PERF_MODE").as_deref() == Ok("1");
    let parent_win = std::env::var("SHADES_PARENT_WIN")
        .ok()
        .and_then(|s| s.parse::<isize>().ok());
    let overlay = std::env::var("SHADES_OVERLAY").as_deref() == Ok("1");
    let mut track_win = std::env::var("SHADES_TRACK_WIN")
        .ok()
        .and_then(|s| s.parse::<isize>().ok());
    let track_foreground_win = std::env::var("SHADES_TRACK_FOREGROUND_WIN").as_deref() == Ok("1");
    let maximized = std::env::var("SHADES_MAXIMIZED").as_deref() == Ok("1");

    let last_pos = cache::get_last_pos();

    let event_loop = EventLoop::new();
    let mut window_builder = WindowBuilder::new()
        .with_title("Shades")
        .with_visible(false)
        .with_decorations(show_decoration)
        .with_always_on_top(always_on_top)
        .with_maximized(maximized);
    if let Some((pos, size)) = last_pos {
        println!("restoring pos: {:?}", &pos);
        window_builder = window_builder.with_position(pos).with_inner_size(size);
    }

    let window = window_builder
        .build(&event_loop)
        .expect("Could not build window");

    let id = window.id();

    win::hide_from_capture(&window).expect("could not hide window from capture");

    use winit::platform::windows::WindowExtWindows;
    println!("hwnd={:?}, pid={}", window.hwnd(), std::process::id());

    if let Some(parent) = parent_win {
        // win::set_child(&window);
        win::set_parent(&window, parent);
    }

    let (pix_sender, pix_receiver) = std::sync::mpsc::sync_channel(1);
    // TODO: create capture thread
    let window = Arc::new(window);
    let winref = window.clone();
    std::thread::spawn(move || {
        // std::thread::sleep(Duration::from_millis(500));
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
            }
        }
    });

    if track_win.is_none() && track_foreground_win {
        track_win = Some(win::get_foreground_hwnd());
    }

    let request_close = Arc::new(AtomicBool::new(false));
    if let Some(hwnd) = track_win {
        let window = Arc::clone(&window);
        let request_close = Arc::clone(&request_close);
        win::track(hwnd, move |event| match event {
            Some(event) => match event {
                win::TrackEvent::Size(size) => window.set_inner_size(size),
                win::TrackEvent::Position(pos) => window.set_outer_position(pos),
            },
            None => request_close.store(true, Ordering::Relaxed),
        });
    }

    let mut pix = pix_receiver.recv().unwrap();
    println!("found {} x {} pixels", pix.width, pix.height);

    let mut pixels = {
        let window_size = window.as_ref().inner_size();
        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());
        Pixels::new(window_size.width, window_size.height, surface_texture)?
    };

    window.request_redraw();

    window.set_visible(true);
    if overlay {
        win::set_transparent(&window);
    }
    win::set_layered(&window);

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
                    } else {
                        break;
                    }
                }
                tmp
            };
            if let Some(pix) = pix_opt {
                let target_width = window.inner_size().width as usize;
                let target_height = window.inner_size().height as usize;
                if target_width == 0 || target_height == 0 {
                    return;
                }
                let src_width = pix.width as i32;
                let mut offset_x = 0;
                let mut offset_y = 0;
                if let Ok(pos) = window.inner_position() {
                    offset_x = pos.x;
                    offset_y = pos.y;
                }
                let max_j = (pix.width * pix.height - 1) as i32;
                {
                    let data = pix.data.lock().unwrap();
                    let avg_rgb = data
                        .chunks_exact(src_width as usize * 4)
                        .skip(max(0, offset_y) as usize)
                        .take(target_height)
                        .map(|row| {
                            let x_left = max(0, min(src_width as i32, offset_x));
                            let x_right =
                                max(0, min(src_width as i32, target_width as i32 + offset_x));
                            row[x_left as usize * 4..x_right as usize * 4]
                                .iter()
                                .map(|&d| d as f32)
                                .sum::<f32>()
                        })
                        .sum::<f32>()
                        / (target_width * target_height * 4) as f32;

                    // TODO: use effect intensity instead
                    let do_invert = avg_rgb > 150.0;

                    for (i, pixel) in pixels.get_frame().chunks_exact_mut(4).enumerate() {
                        let col = (i % target_width) as i32;
                        let row = (i / target_width) as i32;
                        let j = max(0, min(max_j, (row + offset_y) * src_width + col + offset_x))
                            as usize;

                        // TODO: use color-preserving invert
                        // TODO: alternate darkening strategy
                        if do_invert {
                            pixel[2] = 255 - data[j * 4];
                            pixel[1] = 255 - data[j * 4 + 1];
                            pixel[0] = 255 - data[j * 4 + 2];
                        } else {
                            pixel[2] = data[j * 4];
                            pixel[1] = data[j * 4 + 1];
                            pixel[0] = data[j * 4 + 2];
                        }
                        hasher.write(pixel);
                    }
                }
                let frame = pixels.get_frame();
                let flash = (cnt % 16) << 4;
                if target_width > 10 && target_height > 10 {
                    for j in 0..10 {
                        for i in 0..10 {
                            let k = (i + j * target_width as usize) * 4;
                            frame[k] ^= flash;
                        }
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
            } if window_id == id => {
                cache::save_pos(window.outer_position().ok(), window.inner_size());
                *control_flow = ControlFlow::Exit
            }
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
        if request_close.load(Ordering::Relaxed) {
            cache::save_pos(window.outer_position().ok(), window.inner_size());
            *control_flow = ControlFlow::Exit;
        }
    });
}
