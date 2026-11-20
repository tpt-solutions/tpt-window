// SPDX-License-Identifier: MIT OR Apache-2.0
//! Window icons, as straight (non-premultiplied) RGBA pixels.

use std::sync::Arc;

/// Why an icon could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconError {
    /// The width or height is zero.
    Empty,
    /// The pixel buffer is not `width * height * 4` bytes long.
    BadLength {
        /// The byte count expected from the dimensions.
        expected: usize,
        /// The byte count given.
        got: usize,
    },
}

impl core::fmt::Display for IconError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IconError::Empty => f.write_str("icon has no pixels"),
            IconError::BadLength { expected, got } => {
                write!(f, "icon needs {expected} bytes of RGBA, got {got}")
            }
        }
    }
}

impl std::error::Error for IconError {}

/// A window icon: `width * height` pixels, four bytes each, red, green, blue, alpha, rows from
/// the top. Cheap to clone, since the pixels are shared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}

impl Icon {
    /// Builds an icon from RGBA pixels, checking the length.
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, IconError> {
        if width == 0 || height == 0 {
            return Err(IconError::Empty);
        }
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected {
            return Err(IconError::BadLength {
                expected,
                got: rgba.len(),
            });
        }
        Ok(Self {
            width,
            height,
            rgba: rgba.into(),
        })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The RGBA pixels, rows from the top.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_must_match_the_dimensions() {
        assert!(Icon::from_rgba(2, 2, vec![0; 16]).is_ok());
        assert_eq!(
            Icon::from_rgba(2, 2, vec![0; 15]),
            Err(IconError::BadLength {
                expected: 16,
                got: 15
            })
        );
        assert_eq!(Icon::from_rgba(0, 4, Vec::new()), Err(IconError::Empty));
    }
}
