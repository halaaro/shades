#[cfg(windows)]
mod win;

#[cfg(unix)]
mod linux;
use std::sync::{Arc, Mutex};

pub use linux::*;

#[derive(Default)]
pub struct Screenshot {
    pub data: Arc<Mutex<Vec<u8>>>,
    pub width: u32,
    pub height: u32,
}
