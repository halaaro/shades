mod cache;
mod record;
mod render;

// TODO: implement cross-platform wrapper instead
#[cfg(windows)]
mod win;

use std::env;

use log::{debug, info, warn};
use record::ScreenRecorder;
use render::Renderer;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, WindowLevel},
};

use pixels::{Error, Pixels, SurfaceTexture};

pub fn main() -> Result<(), Error> {
    env_logger::init_from_env("SHADES_LOG");
    let show_decoration = !env_true("SHADES_NO_WIN_DECORATION").unwrap_or(false);
    debug!("show_decoration={show_decoration}");
    let always_on_top = !env_true("SHADES_NO_ALWAYS_ON_TOP").unwrap_or(false);
    debug!("always_on_top={always_on_top}");

    #[cfg(windows)]
    {
        let parent_win = std::env::var("SHADES_PARENT_WIN")
            .ok()
            .and_then(|s| s.parse::<isize>().ok());
        let overlay = std::env::var("SHADES_OVERLAY").as_deref() == Ok("1");
        let track_win = std::env::var("SHADES_TRACK_WIN")
            .ok()
            .and_then(|s| s.parse::<isize>().ok());
        let track_foreground_win =
            std::env::var("SHADES_TRACK_FOREGROUND_WIN").as_deref() == Ok("1");
    }
    let maximized = env_true("SHADES_MAXIMIZED").unwrap_or(false);
    debug!("maximized={maximized}");

    let last_pos = cache::get_last_pos();
    debug!("last_pos={last_pos:?}");

    let event_loop = EventLoop::new();
    let mut window_builder = WindowBuilder::new()
        .with_title("Shades")
        .with_visible(false)
        .with_decorations(show_decoration)
        .with_window_level(if always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        })
        .with_maximized(maximized);
    if let Some((pos, size)) = last_pos {
        debug!("restoring pos: {:?}", &pos);
        window_builder = window_builder.with_position(pos).with_inner_size(size);
    }

    let window = window_builder
        .build(&event_loop)
        .expect("Could not build window");

    if let Err(e) = window.set_cursor_hittest(false) {
        warn!("Error setting cursor hittest: {e}");
    }
    #[cfg(windows)]
    {
        win::hide_from_capture(&window).expect("could not hide window from capture");

        use winit::platform::windows::WindowExtWindows;
        debug!("hwnd={:?}, pid={}", window.hwnd(), std::process::id());

        if let Some(parent) = parent_win {
            win::set_parent(&window, parent);
        }
    }

    let mut recorder = ScreenRecorder::capture_primary().expect("could not capture primary");

    #[cfg(windows)]
    {
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
    }

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(window_size.width, window_size.height, surface_texture)?
    };
    window.request_redraw();

    window.set_visible(true);
    #[cfg(windows)]
    {
        if overlay {
            win::set_transparent(&window);
        }
        win::set_layered(&window);
    }

    let mut renderer = Renderer::new();
    renderer.handle_resize(pixels.frame_mut());
    event_loop.run(move |event, _, control_flow| {
        if let Event::RedrawRequested(_) = event {
            let pix = recorder.next().unwrap();
            renderer.draw(&window, &pix, pixels.frame_mut());
            if pixels
                .render()
                .map_err(|e| panic!("pixels.render() failed: {:?}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => {
                cache::save_pos(window.outer_position().ok(), window.inner_size());
                *control_flow = ControlFlow::Exit
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if window_id == window.id() && size.width > 0 && size.height > 0 => {
                debug!("resized to {:?}!", size);

                pixels.resize_surface(size.width, size.height).unwrap();
                pixels.resize_buffer(size.width, size.height).unwrap();
                renderer.handle_resize(pixels.frame_mut());
                window.request_redraw();
            }
            _ => (),
        }
        #[cfg(windows)]
        {
            if request_close.load(Ordering::Relaxed) {
                cache::save_pos(window.outer_position().ok(), window.inner_size());
                *control_flow = ControlFlow::Exit;
            }
        }
        window.request_redraw();
    });
}

fn env_true(name: &str) -> Option<bool> {
    env::var(name)
        .map(|mut v| {
            v.make_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "y" | "yes")
        })
        .ok()
}
