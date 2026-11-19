// SPDX-License-Identifier: MIT OR Apache-2.0
//! Physical (device pixel) and logical (DPI-independent) sizes and positions.

macro_rules! pair {
    ($name:ident, $a:ident, $b:ident) => {
        /// A pair of values; see the module docs.
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub struct $name<P> {
            #[allow(missing_docs)]
            pub $a: P,
            #[allow(missing_docs)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
