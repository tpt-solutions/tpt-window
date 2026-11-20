// SPDX-License-Identifier: MIT OR Apache-2.0
//! Physical (device pixel) and logical (DPI-independent) sizes and positions.

macro_rules! pair {
    ($name:ident, $a:ident, $b:ident) => {
        /// A pair of values; see the module docs.
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub struct $name<P> {
            #[doc = concat!("The `", stringify!($a), "` component.")]
            pub $a: P,
            #[doc = concat!("The `", stringify!($b), "` component.")]
            pub $b: P,
        }
        impl<P> $name<P> {
            /// Creates a new value.
            pub const fn new($a: P, $b: P) -> Self {
                Self { $a, $b }
            }
        }
    };
}

pair!(PhysicalSize, width, height);
pair!(LogicalSize, width, height);
pair!(PhysicalPosition, x, y);
pair!(LogicalPosition, x, y);

impl PhysicalSize<u32> {
    /// Converts to logical units using `scale_factor`.
    pub fn to_logical(self, scale_factor: f64) -> LogicalSize<f64> {
        LogicalSize::new(
            f64::from(self.width) / scale_factor,
            f64::from(self.height) / scale_factor,
        )
    }
}

impl LogicalSize<f64> {
    /// Converts to physical pixels using `scale_factor` (rounded, clamped at 0).
    pub fn to_physical(self, scale_factor: f64) -> PhysicalSize<u32> {
        PhysicalSize::new(
            (self.width * scale_factor).round().max(0.0) as u32,
            (self.height * scale_factor).round().max(0.0) as u32,
        )
    }
}

impl LogicalPosition<f64> {
    /// Converts to physical pixels using `scale_factor`.
    pub fn to_physical(self, scale_factor: f64) -> PhysicalPosition<i32> {
        PhysicalPosition::new(
            (self.x * scale_factor).round() as i32,
            (self.y * scale_factor).round() as i32,
        )
    }
}

/// A size given either in physical pixels or in logical units. Setters take `impl Into<Size>`,
/// so both `PhysicalSize` and `LogicalSize` can be passed directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Device pixels.
    Physical(PhysicalSize<u32>),
    /// DPI-independent units, converted with the window's scale factor.
    Logical(LogicalSize<f64>),
}

impl Size {
    /// The size in device pixels at `scale_factor`.
    pub fn to_physical(self, scale_factor: f64) -> PhysicalSize<u32> {
        match self {
            Size::Physical(p) => p,
            Size::Logical(l) => l.to_physical(scale_factor),
        }
    }
}

impl From<PhysicalSize<u32>> for Size {
    fn from(size: PhysicalSize<u32>) -> Self {
        Size::Physical(size)
    }
}

impl From<LogicalSize<f64>> for Size {
    fn from(size: LogicalSize<f64>) -> Self {
        Size::Logical(size)
    }
}

/// A position given either in physical pixels or in logical units. See [`Size`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Position {
    /// Device pixels.
    Physical(PhysicalPosition<i32>),
    /// DPI-independent units, converted with the window's scale factor.
    Logical(LogicalPosition<f64>),
}

impl Position {
    /// The position in device pixels at `scale_factor`.
    pub fn to_physical(self, scale_factor: f64) -> PhysicalPosition<i32> {
        match self {
            Position::Physical(p) => p,
            Position::Logical(l) => l.to_physical(scale_factor),
        }
    }
}

impl From<PhysicalPosition<i32>> for Position {
    fn from(pos: PhysicalPosition<i32>) -> Self {
        Position::Physical(pos)
    }
}

impl From<LogicalPosition<f64>> for Position {
    fn from(pos: LogicalPosition<f64>) -> Self {
        Position::Logical(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_and_position_convert_at_the_scale() {
        // Logical units are scaled; physical values pass through unchanged.
        assert_eq!(
            Size::from(LogicalSize::new(100.0, 50.0)).to_physical(1.5),
            PhysicalSize::new(150, 75)
        );
        assert_eq!(
            Size::from(PhysicalSize::new(640_u32, 480_u32)).to_physical(2.0),
            PhysicalSize::new(640, 480)
        );
        assert_eq!(
            Position::from(LogicalPosition::new(10.0, -4.0)).to_physical(2.0),
            PhysicalPosition::new(20, -8)
        );
        assert_eq!(
            Position::from(PhysicalPosition::new(3, 4)).to_physical(3.0),
            PhysicalPosition::new(3, 4)
        );
    }

    #[test]
    fn round_trip() {
        let l = LogicalSize::new(800.0, 600.0);
        let p = l.to_physical(1.5);
        assert_eq!(p, PhysicalSize::new(1200, 900));
        assert_eq!(p.to_logical(1.5), l);
    }

    #[test]
    fn negative_clamps() {
        assert_eq!(
            LogicalSize::new(-5.0, 2.0).to_physical(1.0),
            PhysicalSize::new(0, 2)
        );
    }
}
