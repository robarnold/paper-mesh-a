// SPDX-License-Identifier: GPL-3.0-only
//! ED047TC1 framebuffer and scan encoding.
//! The firmware supplies PSRAM storage and controls the panel bus.
//! See ../README.md for source attribution and waveform limits.

#![no_std]

use core::convert::Infallible;
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

pub const WIDTH: usize = 960;
pub const HEIGHT: usize = 540;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT / 2;
pub const ROW_BYTES: usize = WIDTH / 4;
pub const STORAGE_BYTES: usize = FRAME_BYTES * 2;

// Each value gives CKV high time in 0.1 microsecond units.
pub const PHASE_TICKS: [u16; 15] = [
    30, 30, 20, 20, 30, 30, 30, 40, 40, 50, 50, 50, 100, 200, 300,
];

// The reference DU waveform uses five identical 100 microsecond drive phases.
pub const PARTIAL_PHASE_TICKS: [u16; 5] = [1000; 5];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Damage {
    pub area: Rectangle,
    pub pixels: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    Full,
    Partial(Damage),
    Unchanged,
}

/// The last frame whose scan and supply shutdown completed successfully.
pub struct FrameHistory<'a> {
    bytes: &'a mut [u8; FRAME_BYTES],
    valid: bool,
}

impl<'a> FrameHistory<'a> {
    pub fn new(bytes: &'a mut [u8; FRAME_BYTES]) -> Self {
        bytes.fill(0xff);
        Self {
            bytes,
            valid: false,
        }
    }

    /// Select partial refresh only for known black-to-white or white-to-black changes.
    /// Unchanged gray pixels always receive neutral drive commands.
    pub fn plan(&self, target: &Framebuffer<'_>) -> Update {
        if !self.valid {
            return Update::Full;
        }
        let mut minimum = Point::new(WIDTH as i32, HEIGHT as i32);
        let mut maximum = Point::zero();
        let mut pixels = 0;
        for (index, (&previous, &next)) in self.bytes.iter().zip(target.bytes.iter()).enumerate() {
            if previous == next {
                continue;
            }
            for pixel in 0..2 {
                let from = (previous >> (4 * pixel)) & 15;
                let to = (next >> (4 * pixel)) & 15;
                if from == to {
                    continue;
                }
                if monochrome_drive(from, to).is_none() {
                    return Update::Full;
                }
                let x = ((index * 2 + pixel) % WIDTH) as i32;
                let y = (index * 2 / WIDTH) as i32;
                minimum.x = minimum.x.min(x);
                minimum.y = minimum.y.min(y);
                maximum.x = maximum.x.max(x);
                maximum.y = maximum.y.max(y);
                pixels += 1;
            }
        }
        if pixels == 0 {
            Update::Unchanged
        } else {
            Update::Partial(Damage {
                area: Rectangle::with_corners(minimum, maximum),
                pixels,
            })
        }
    }

    /// Invalidate history before a scan. An interrupted update requires a full refresh.
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Identify only a frame whose refresh and shutdown completed successfully.
    pub fn checksum(&self) -> Option<u32> {
        self.valid.then(|| {
            self.bytes.iter().fold(0x811c9dc5_u32, |hash, byte| {
                (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
            })
        })
    }

    pub fn commit(&mut self, target: &Framebuffer<'_>) {
        self.bytes.copy_from_slice(target.bytes);
        self.valid = true;
    }

    /// Encode one native row. Return false if a changed pixel requires grayscale drive.
    pub fn encode_partial_row(
        &self,
        target: &Framebuffer<'_>,
        row: usize,
        output: &mut [u8; ROW_BYTES],
    ) -> bool {
        assert!(row < HEIGHT);
        let start = row * WIDTH / 2;
        output.fill(0);
        for (byte, destination) in output.iter_mut().enumerate() {
            for pixel in 0..4 {
                let index = start + byte * 2 + pixel / 2;
                let shift = 4 * (pixel & 1);
                let from = (self.bytes[index] >> shift) & 15;
                let to = (target.bytes[index] >> shift) & 15;
                let Some(drive) = monochrome_drive(from, to) else {
                    return false;
                };
                *destination |= drive << (2 * pixel);
            }
        }
        true
    }
}

fn monochrome_drive(from: u8, to: u8) -> Option<u8> {
    match (from, to) {
        (from, to) if from == to => Some(0),
        (15, 0) => Some(1),
        (0, 15) => Some(2),
        _ => None,
    }
}

pub struct Framebuffer<'a> {
    bytes: &'a mut [u8; FRAME_BYTES],
}

impl<'a> Framebuffer<'a> {
    pub fn new(bytes: &'a mut [u8; FRAME_BYTES]) -> Self {
        bytes.fill(0xff);
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }

    /// FNV-1a identifies the packed frame in USB output.
    pub fn checksum(&self) -> u32 {
        self.bytes.iter().fold(0x811c9dc5_u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
        })
    }

    /// Encode four pixels per byte for one black-on-white phase.
    /// The LCD pin permutation supplies the panel's pixel order.
    pub fn encode_row(&self, row: usize, phase: usize, output: &mut [u8; ROW_BYTES]) {
        assert!(row < HEIGHT && phase < PHASE_TICKS.len());
        let threshold = 15 - phase as u8;
        let start = row * WIDTH / 2;
        for (source, destination) in self.bytes[start..start + WIDTH / 2]
            .chunks_exact(2)
            .zip(output)
        {
            let shades = [
                source[0] & 15,
                source[0] >> 4,
                source[1] & 15,
                source[1] >> 4,
            ];
            *destination = shades.iter().enumerate().fold(0, |drive, (pixel, shade)| {
                drive | (u8::from(*shade < threshold) << (pixel * 2))
            });
        }
    }
}

impl OriginDimensions for Framebuffer<'_> {
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl DrawTarget for Framebuffer<'_> {
    type Color = Gray4;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(point, shade) in pixels {
            if self.bounding_box().contains(point) {
                let index = (point.y as usize * WIDTH + point.x as usize) / 2;
                let shift = (point.x as usize & 1) * 4;
                self.bytes[index] = (self.bytes[index] & !(15 << shift)) | (shade.luma() << shift);
            }
        }
        Ok(())
    }

    fn clear(&mut self, shade: Gray4) -> Result<(), Infallible> {
        self.bytes.fill(shade.luma() * 17);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec;

    #[test]
    fn history_requires_completion_and_rejects_grayscale_changes() {
        let mut bytes = vec![0; FRAME_BYTES];
        let mut previous = vec![0; FRAME_BYTES];
        let mut frame = Framebuffer::new(bytes.as_mut_slice().try_into().unwrap());
        let mut history = FrameHistory::new(previous.as_mut_slice().try_into().unwrap());
        assert_eq!(history.checksum(), None);
        assert_eq!(history.plan(&frame), Update::Full);
        history.commit(&frame);
        let completed = frame.checksum();
        assert_eq!(history.checksum(), Some(completed));
        assert_eq!(history.plan(&frame), Update::Unchanged);
        frame
            .draw_iter([
                Pixel(Point::new(0, 0), Gray4::BLACK),
                Pixel(Point::new(959, 539), Gray4::BLACK),
            ])
            .unwrap();
        assert_eq!(
            history.plan(&frame),
            Update::Partial(Damage {
                area: Rectangle::new(Point::zero(), Size::new(960, 540)),
                pixels: 2,
            })
        );
        assert_eq!(history.checksum(), Some(completed));
        assert_ne!(history.checksum(), Some(frame.checksum()));
        history.invalidate();
        assert_eq!(history.checksum(), None);
        assert_eq!(history.plan(&frame), Update::Full);
        history.commit(&frame);
        assert_eq!(history.checksum(), Some(frame.checksum()));
        assert_eq!(history.plan(&frame), Update::Unchanged);
        frame
            .draw_iter([Pixel(Point::new(2, 4), Gray4::new(7))])
            .unwrap();
        assert_eq!(history.plan(&frame), Update::Full);
        let mut row = [0; ROW_BYTES];
        assert!(!history.encode_partial_row(&frame, 4, &mut row));
        history.commit(&frame);
        frame
            .draw_iter([Pixel(Point::new(2, 4), Gray4::WHITE)])
            .unwrap();
        assert_eq!(history.plan(&frame), Update::Full);
        assert!(!history.encode_partial_row(&frame, 4, &mut row));
    }

    #[test]
    fn partial_encoding_masks_neighbors_and_other_rows() {
        let mut bytes = vec![0; FRAME_BYTES];
        let mut previous = vec![0; FRAME_BYTES];
        let mut frame = Framebuffer::new(bytes.as_mut_slice().try_into().unwrap());
        let mut history = FrameHistory::new(previous.as_mut_slice().try_into().unwrap());
        for shade in 0..16 {
            frame.clear(Gray4::new(shade)).unwrap();
            frame
                .draw_iter([
                    Pixel(Point::new(3, 4), Gray4::BLACK),
                    Pixel(Point::new(4, 4), Gray4::WHITE),
                ])
                .unwrap();
            history.commit(&frame);
            frame
                .draw_iter([
                    Pixel(Point::new(3, 4), Gray4::WHITE),
                    Pixel(Point::new(4, 4), Gray4::BLACK),
                ])
                .unwrap();
            assert_eq!(
                history.plan(&frame),
                Update::Partial(Damage {
                    area: Rectangle::new(Point::new(3, 4), Size::new(2, 1)),
                    pixels: 2,
                })
            );
            let mut row = [0xff; ROW_BYTES];
            assert!(history.encode_partial_row(&frame, 4, &mut row));
            assert_eq!(row[0], 0x80);
            assert_eq!(row[1], 0x01);
            assert!(row[2..].iter().all(|byte| *byte == 0));
            assert!(history.encode_partial_row(&frame, 3, &mut row));
            assert!(row.iter().all(|byte| *byte == 0));
        }
    }

    #[test]
    fn pixel_packing_clips_and_preserves_neighbors() {
        let mut bytes = vec![0; FRAME_BYTES];
        let mut frame = Framebuffer::new(bytes.as_mut_slice().try_into().unwrap());
        frame
            .draw_iter([
                Pixel(Point::new(-1, 0), Gray4::BLACK),
                Pixel(Point::new(960, 0), Gray4::BLACK),
                Pixel(Point::new(0, 540), Gray4::BLACK),
                Pixel(Point::new(0, 0), Gray4::new(2)),
                Pixel(Point::new(1, 0), Gray4::new(9)),
                Pixel(Point::new(959, 539), Gray4::new(4)),
            ])
            .unwrap();
        assert_eq!(frame.bytes()[0], 0x92);
        assert_eq!(frame.bytes()[1], 0xff);
        assert_eq!(frame.bytes()[480], 0xff);
        assert_eq!(frame.bytes()[FRAME_BYTES - 1], 0x4f);
    }

    #[test]
    fn all_shades_have_the_expected_drive_duration_and_pixel_order() {
        let mut bytes = vec![0; FRAME_BYTES];
        let mut frame = Framebuffer::new(bytes.as_mut_slice().try_into().unwrap());
        frame
            .draw_iter((0..16).map(|shade| Pixel(Point::new(shade, 0), Gray4::new(shade as u8))))
            .unwrap();
        let mut row = [0; ROW_BYTES];
        let mut counts = [0; 16];
        for phase in 0..15 {
            frame.encode_row(0, phase, &mut row);
            for shade in 0..16 {
                let drive = (row[shade / 4] >> (2 * (shade % 4))) & 3;
                assert_eq!(drive, u8::from(shade < 15 - phase));
                counts[shade] += drive;
            }
            assert!(row[4..].iter().all(|byte| *byte == 0));
        }
        assert_eq!(
            counts,
            [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
        );
    }
}
