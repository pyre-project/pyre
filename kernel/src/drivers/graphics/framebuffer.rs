#![allow(dead_code)]

use crate::drivers::graphics::color::{Color8i, Colors};
use libkernel::Size;
use spin::{Mutex, RwLock};

#[repr(C)]
pub struct FramebufferDriver {
    framebuffer: Mutex<*mut Color8i>,
    backbuffer: RwLock<*mut Color8i>,
    dimensions: Size,
    scanline_width: usize,
}

impl FramebufferDriver {
    pub fn init(buffer_addr: libkernel::PhysAddr, dimensions: Size, scanline_width: usize) -> Self {
        let pixel_len = scanline_width * dimensions.height();
        let byte_len = pixel_len * core::mem::size_of::<Color8i>();

        let framebuffer = unsafe {
            let start_frame_index = (buffer_addr.as_u64() / 0x1000) as usize;
            let end_frame_index = start_frame_index + ((byte_len + 0xFFF) / 0x1000);
            let mmio_frames = libkernel::memory::global_memory()
                .acquire_frames(
                    start_frame_index..end_frame_index,
                    libkernel::memory::FrameState::MMIO,
                )
                .unwrap();

            libkernel::alloc_to!(mmio_frames)
        };

        info!("{:?} {}", dimensions, scanline_width);

        Self {
            framebuffer: Mutex::new(framebuffer),
            backbuffer: RwLock::new(libkernel::alloc!(byte_len)),
            dimensions,
            scanline_width,
        }
    }

    pub fn write_pixel(&self, xy: (usize, usize), color: Color8i) {
        if self.contains_point(xy) {
            unsafe {
                self.backbuffer
                    .write()
                    .add(self.point_to_offset(xy))
                    .write_volatile(color)
            };
        } else {
            panic!("point lies without framebuffer");
        }
    }

    pub fn clear(&mut self, color: Color8i) {
        let backbuffer = self.backbuffer.write();
        for y in 0..self.dimensions().height() {
            for x in 0..self.dimensions().width() {
                unsafe {
                    backbuffer
                        .add(self.point_to_offset((x, y)))
                        .write_volatile(color)
                }
            }
        }
    }

    /// Copy backbuffer to frontbuffer and zero backbuffer
    pub fn flush_pixels(&mut self) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                *self.backbuffer.read(),
                *self.framebuffer.lock(),
                self.dimensions().len(),
            )
        };

        self.clear(Colors::Black.into());
    }

    pub const fn dimensions(&self) -> Size {
        self.dimensions
    }

    const fn point_to_offset(&self, point: (usize, usize)) -> usize {
        (point.1 * self.scanline_width) + point.0
    }

    const fn contains_point(&self, point: (usize, usize)) -> bool {
        point.0 < self.dimensions().width() && point.1 < self.dimensions().height()
    }
}
