use crate::record::Screenshot;
use log::debug;
use std::cmp::{max, min};
use winit::window::Window;

pub(crate) struct Renderer {
    cnt: usize,
    dir: i32,
}

impl Renderer {
    pub(crate) fn new() -> Self {
        Renderer { cnt: 0, dir: 1 }
    }

    pub(crate) fn draw(&mut self, window: &Window, pix: &Screenshot, frame: &mut [u8]) {
        let target_height = window.inner_size().height as usize;
        let target_width = window.inner_size().width as usize;
        if target_width == 0 || target_height == 0 {
            return;
        }
        let src_width = pix.width as i32;
        let mut offset_x = 100;
        let mut offset_y = 100;
        if let Ok(pos) = window.inner_position() {
            offset_x = pos.x;
            offset_y = pos.y;
        }
        let max_j = (pix.width * pix.height - 1) as i32;

        let data = pix.data.lock().unwrap();
        let avg_rgb = data
            .chunks_exact(src_width as usize * 4)
            .skip(max(0, offset_y) as usize)
            .take(target_height)
            .map(|row| {
                let x_left = max(0, min(src_width, offset_x));
                let x_right = max(0, min(src_width, target_width as i32 + offset_x));
                row[x_left as usize * 4..x_right as usize * 4]
                    .iter()
                    .map(|&d| d as f32)
                    .sum::<f32>()
            })
            .sum::<f32>()
            / (target_width * target_height * 4) as f32;

        // TODO: use effect intensity instead
        let do_invert = avg_rgb > 150.0;

        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let col = (i % target_width) as i32;
            let row = (i / target_width) as i32;
            let j = max(0, min(max_j, (row + offset_y) * src_width + col + offset_x)) as usize;
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
        }

        let flash = (self.cnt % 16) << 4;
        if target_width > 10 && target_height > 10 {
            for j in 0..10 {
                for i in 0..10 {
                    let k = (i + j * target_width) * 4;
                    frame[k] ^= flash as u8;
                }
            }
        }
        if self.dir == 1 {
            self.cnt += 1;
        } else {
            self.cnt -= 1;
        }
        if self.cnt == 0 || self.cnt == 15 {
            self.dir = -self.dir
        };

        if self.cnt == 0 {
            debug!("avg_rgb={avg_rgb}");
            debug!("do_invert={do_invert}");
        }
    }

    pub(crate) fn handle_resize(&self, frame_mut: &mut [u8]) {
        frame_mut.chunks_exact_mut(4).for_each(|p| p[3] = 0xff)
    }
}
