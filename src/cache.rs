use std::{env::temp_dir, fs};

use winit::dpi::{PhysicalPosition, PhysicalSize};

const CACHE_NAME: &str = ".shades.cache";

pub(crate) fn get_last_pos() -> Option<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    let mut cache_path = temp_dir();
    cache_path.push(CACHE_NAME);
    match fs::read_to_string(cache_path).map(|s| {
        println!("read cache: {}", &s);
        s.split(',')
            .map(|val| val.parse::<i32>().unwrap())
            .collect::<Vec<_>>()
    }) {
        Ok(v) if v.len() == 4 => Some((
            PhysicalPosition::new(v[0], v[1]),
            PhysicalSize::new(v[2] as u32, v[3] as u32),
        )),
        _ => None,
    }
}

pub(crate) fn save_pos(outer_position: Option<PhysicalPosition<i32>>, size: PhysicalSize<u32>) {
    let pos = match outer_position {
        Some(p) => p,
        _ => PhysicalPosition::new(0, 0),
    };
    let mut cache_path = temp_dir();
    cache_path.push(CACHE_NAME);
    println!("writing to {:?}", &cache_path);
    fs::write(
        cache_path,
        format!("{},{},{},{}", pos.x, pos.y, size.width, size.height),
    )
    .expect("could not write to cache");
}
