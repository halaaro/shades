use std::sync::{Arc, Mutex};

use super::Screenshot;
use scrap::*;

pub struct ScreenRecorder {
    capturer: Capturer,
}
impl ScreenRecorder {
    pub(crate) fn capture_primary() -> Result<Self, &'static str> {
        let display = Display::primary().or(Err("Couldn't find primary display."))?;
        let capturer = Capturer::new(display).or(Err("Couldn't begin capture."))?;
        Ok(ScreenRecorder { capturer })
    }

    pub(crate) fn next(&mut self) -> Result<Screenshot, &'static str> {
        let (width, height) = (self.capturer.width() as u32, self.capturer.height() as u32);
        let data = Arc::new(Mutex::new(
            self.capturer
                .frame()
                .or(Err("error capturing frame"))?
                .to_vec(),
        ));
        Ok(Screenshot {
            width,
            height,
            data,
        })
    }
}
