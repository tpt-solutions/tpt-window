// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fixed-capacity inline UTF-8 text, so text events stay `Copy` and never allocate.

/// UTF-8 text stored inline in at most `N` bytes (`N <= 255`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InlineText<const N: usize> {
    len: u8,
    buf: [u8; N],
}

impl<const N: usize> InlineText<N> {
    const _CHECK: () = assert!(N > 0 && N <= 255, "InlineText capacity must be 1..=255");

    /// Empty text.
    pub const fn empty() -> Self {
        let () = Self::_CHECK;
        Self {
            len: 0,
            buf: [0; N],
        }
    }

    /// Copies as much of `s` as fits without splitting a character and returns
    /// the text plus the unconsumed remainder (for chunking long input).
    pub fn take_chunk(s: &str) -> (Self, &str) {
        let mut end = s.len().min(N);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        let mut out = Self::empty();
        out.buf[..end].copy_from_slice(&s.as_bytes()[..end]);
        out.len = end as u8;
        (out, &s[end..])
    }

    /// Copies `s`, truncating at a character boundary if it does not fit.
    pub fn new(s: &str) -> Self {
        Self::take_chunk(s).0
    }

    /// The stored text.
    pub fn as_str(&self) -> &str {
        // Invariant: only whole UTF-8 characters are ever copied in.
        core::str::from_utf8(&self.buf[..usize::from(self.len)]).unwrap_or("")
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<const N: usize> Default for InlineText<N> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<const N: usize> core::fmt::Debug for InlineText<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// Inline text size used by IME events.
pub type ImeText = InlineText<32>;

/// Input-method events. Long strings are split into chunks with `continued = true`
/// on every chunk except the last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImeEvent {
    /// IME became active.
    Enabled,
    /// Composition in progress.
    Preedit {
        /// Current composition text (one chunk).
        text: ImeText,
        /// More chunks of this same preedit follow.
        continued: bool,
    },
    /// Final text to insert.
    Commit {
        /// Committed text (one chunk).
        text: ImeText,
        /// More chunks of this same commit follow.
        continued: bool,
    },
    /// IME became inactive.
    Disabled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits() {
        assert_eq!(InlineText::<8>::new("héllo").as_str(), "héllo");
    }

    #[test]
    fn never_splits_chars() {
        // 'é' is 2 bytes; capacity 4 fits "hél" (4 bytes) then remainder.
        let (t, rest) = InlineText::<4>::take_chunk("hélo");
        assert_eq!(t.as_str(), "hél");
        assert_eq!(rest, "o");
        let (t, rest) = InlineText::<2>::take_chunk("日本");
        assert_eq!((t.as_str(), rest), ("", "日本")); // 3-byte char can't fit
    }
}
