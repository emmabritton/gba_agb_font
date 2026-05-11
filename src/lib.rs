#![no_std]

use crate::full::FullFont;
use crate::printable::PrintableFont;

pub mod full;
pub mod printable;

pub mod prelude {
    pub use crate::AgbFont;
    pub use crate::Lines;
    pub use crate::full::FullFont;
    pub use crate::full_font;
    pub use crate::printable::PrintableFont;
    pub use crate::printable_font;
}

/// Iterator over visual lines of text, returned by [`AgbFont::lines`].
///
/// Each item is `(line_slice, line_width_px)`. Word-wrap boundaries consume the
/// space character; hard-wrap and explicit `\n` boundaries do not consume extra bytes.
pub struct Lines<'a, F: AgbFont + ?Sized> {
    font: &'a F,
    remaining: &'a [u8],
    wrap_px: Option<u32>,
    word_wrap: bool,
}

impl<'a, F: AgbFont + ?Sized> Lines<'a, F> {
    fn next_word_wrapped(&mut self, wrap_px: u32) -> (&'a [u8], u32) {
        let text = self.remaining;
        let mut line_w: u32 = 0;
        let mut i = 0;

        while i < text.len() {
            let c = text[i];
            if c == b'\n' {
                self.remaining = &text[i + 1..];
                return (&text[..i], line_w);
            }
            if c == b' ' {
                let (space_w, next_w, next_end) = self.font.word_lookahead(text, i);
                if next_end > i + 1 && line_w > 0 && line_w + space_w + next_w > wrap_px {
                    self.remaining = &text[i + 1..];
                    return (&text[..i], line_w);
                }
                line_w += space_w;
            } else {
                let char_w = self.font.char_width(c) as u32;
                if i > 0 && line_w + char_w > wrap_px {
                    self.remaining = &text[i..];
                    return (&text[..i], line_w);
                }
                line_w += char_w;
            }
            i += 1;
        }
        self.remaining = &[];
        (text, line_w)
    }
}

impl<'a, F: AgbFont + ?Sized> Iterator for Lines<'a, F> {
    type Item = (&'a [u8], u32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        if self.word_wrap
            && let Some(wrap) = self.wrap_px
        {
            return Some(self.next_word_wrapped(wrap));
        }
        let (width, consumed) = self.font.measure_line(self.remaining, self.wrap_px);
        let ends_newline = consumed > 0 && self.remaining[consumed - 1] == b'\n';
        let line = if ends_newline {
            &self.remaining[..consumed - 1]
        } else {
            &self.remaining[..consumed]
        };
        self.remaining = &self.remaining[consumed..];
        Some((line, width))
    }
}

/// 4bpp bitmap font for the GBA, loaded from a binary font blob.
pub trait AgbFont {
    /// Advance widths in pixels for each character in the font's range.
    fn char_widths(&self) -> &[u8];

    /// Advance width in pixels for character `c`.
    #[inline]
    fn char_width(&self, c: u8) -> u8 {
        debug_assert!(c as usize >= self.char_offset(), "Attempted to get char outside of font bounds");
        self.char_widths()[c as usize - self.char_offset()]
    }

    /// Subtracted from a character's byte value to index into [`char_widths`](AgbFont::char_widths).
    fn char_offset(&self) -> usize;

    /// Raw 4bpp pixel data; 8 pixels packed per `u32`, row-major.
    fn data(&self) -> &[u32];

    /// Height of every glyph in pixels.
    fn glyph_height(&self) -> u32;

    /// Number of `u32`s per glyph
    fn glyph_size(&self) -> usize;

    /// Number of `u32`s per glyph row
    fn row_u32s(&self) -> usize;

    /// Return the pixel data for the glyph corresponding to `c`.
    ///
    /// Panics in debug builds if `c` is outside the font's character range.
    /// In release builds, out-of-range values produce undefined pixel data.
    fn glyph(&self, c: u8) -> &[u32] {
        let idx = c as usize;
        debug_assert!(
            idx >= self.char_offset() && idx < self.char_offset() + self.char_widths().len(),
            "glyph {c} out of font bounds"
        );
        let idx = c as usize - self.char_offset();
        let offset = idx * self.glyph_size();
        unsafe {
            self.data()
                .get_unchecked(offset..offset + self.glyph_size())
        }
    }

    /// Returns an iterator over visual lines of `text`.
    ///
    /// Each item is `(line_slice, line_width_px)`. Set `word_wrap` to `true` to
    /// break at word (space) boundaries instead of character boundaries.
    ///
    /// When `word_wrap` is `true` but `wrap_px` is `None`, word wrapping is silently
    /// skipped and the iterator falls back to character-level breaking (same as
    /// `word_wrap: false`).
    fn lines<'a>(&'a self, text: &'a [u8], wrap_px: Option<u32>, word_wrap: bool) -> Lines<'a, Self>
    where
        Self: Sized,
    {
        Lines {
            font: self,
            remaining: text,
            wrap_px,
            word_wrap,
        }
    }

    /// Mutate `text` in place for word-aware wrapping, replacing spaces with `\n`
    /// where the next word would overflow `wrap_px`.
    ///
    /// Words longer than `wrap_px` are **not** split here — because the slice length
    /// is fixed, inserting a mid-word `\n` would require shifting bytes.
    /// The caller's renderer must handle character-level hard breaks for overlong words.
    /// (`int_draw_text` does this; use [`lines`](AgbFont::lines) instead if you need
    /// a self-contained iterator that handles both cases.)
    fn word_wrap_in_place(&self, text: &mut [u8], wrap_px: u32) {
        let mut line_w: u32 = 0;
        let mut i = 0;
        while i < text.len() {
            let c = text[i];
            if c == b'\n' {
                line_w = 0;
            } else if c == b' ' {
                let (space_w, next_w, next_end) = self.word_lookahead(text, i);
                if next_end > i + 1 && line_w > 0 && line_w + space_w + next_w > wrap_px {
                    text[i] = b'\n';
                    line_w = 0;
                } else {
                    line_w += space_w;
                }
            } else {
                line_w += self.char_width(c) as u32;
            }
            i += 1;
        }
    }

    #[inline]
    fn word_lookahead(&self, text: &[u8], space_at: usize) -> (u32, u32, usize) {
        let space_w = self.char_width(b' ') as u32;
        let next_start = space_at + 1;
        let next_end = text[next_start..]
            .iter()
            .position(|&x| x == b' ' || x == b'\n')
            .map_or(text.len(), |p| next_start + p);
        let next_w: u32 = text[next_start..next_end]
            .iter()
            .map(|&x| self.char_width(x) as u32)
            .sum();
        (space_w, next_w, next_end)
    }

    /// Measures one line of text, returning `(line_width_px, bytes_consumed)`.
    ///
    /// `bytes_consumed` includes the terminating `\n` if present, so
    /// `&text[bytes_consumed..]` always starts the next line. Wrapping
    /// never splits off a zero-width line: the first character always fits.
    fn measure_line(&self, text: &[u8], wrap_at: Option<u32>) -> (u32, usize) {
        let mut width: u32 = 0;
        for (i, &c) in text.iter().enumerate() {
            if c == b'\n' {
                return (width, i + 1);
            }
            let char_w = self.char_width(c) as u32;
            if let Some(max) = wrap_at
                && i > 0
                && width + char_w > max
            {
                return (width, i);
            }
            width += char_w;
        }
        (width, text.len())
    }

    /// Returns the `(width, height)` in pixels required to render `text`, with optional line-wrapping.
    fn size_of(&self, text: &[u8], wrap_at: Option<u32>) -> (u32, u32) {
        if text.is_empty() {
            return (0, 0);
        }
        let mut max_w: u32 = 0;
        let mut total_h: u32 = 0;
        let mut remaining = text;
        loop {
            let (line_w, consumed) = self.measure_line(remaining, wrap_at);
            if line_w > max_w {
                max_w = line_w;
            }
            total_h += self.glyph_height();
            remaining = &remaining[consumed..];
            if remaining.is_empty() {
                break;
            }
        }
        (max_w, total_h)
    }
}

macro_rules! impl_agb_font {
    ($font_class:ident, $offset: expr) => {
        impl AgbFont for $font_class {
            #[inline]
            fn char_widths(&self) -> &[u8] {
                &self.char_widths
            }

            #[inline]
            fn char_offset(&self) -> usize {
                $offset
            }

            #[inline]
            fn data(&self) -> &[u32] {
                &self.data
            }

            #[inline]
            fn glyph_height(&self) -> u32 {
                self.glyph_height
            }

            #[inline]
            fn glyph_size(&self) -> usize {
                self.glyph_size
            }

            #[inline]
            fn row_u32s(&self) -> usize {
                self.row_u32s
            }
        }
    };
}

impl_agb_font!(PrintableFont, 32);
impl_agb_font!(FullFont, 0);

#[cfg(test)]
mod tests {
    use super::*;

    struct MockFont([u8; 95]);

    impl AgbFont for MockFont {
        fn char_widths(&self) -> &[u8] {
            &self.0
        }
        fn char_offset(&self) -> usize {
            32
        }
        fn data(&self) -> &[u32] {
            &[]
        }
        fn glyph_height(&self) -> u32 {
            8
        }
        fn glyph_size(&self) -> usize {
            0
        }
        fn row_u32s(&self) -> usize {
            1
        }
    }

    fn font(w: u8) -> MockFont {
        MockFont([w; 95])
    }

    #[test]
    fn measure_line_empty() {
        assert_eq!(font(6).measure_line(b"", None), (0, 0));
    }

    #[test]
    fn measure_line_no_wrap() {
        // "abc" with width-6 chars = 18px, 3 bytes consumed
        assert_eq!(font(6).measure_line(b"abc", None), (18, 3));
        // "abc" with width-8 chars = 24px, 3 bytes consumed
        assert_eq!(font(8).measure_line(b"abc", None), (24, 3));
    }

    #[test]
    fn measure_line_newline() {
        // "ab\ncd" — stops at \n, consumed includes the \n byte
        assert_eq!(font(6).measure_line(b"ab\ncd", None), (12, 3));
    }

    #[test]
    fn measure_line_hard_wrap() {
        // wrap at 15px with width-6: "ab" = 12px fits, "c" would be 18 — wraps before c
        assert_eq!(font(6).measure_line(b"abc", Some(15)), (12, 2));
    }

    #[test]
    fn measure_line_first_char_always_fits() {
        // Even if wrap_px is tiny, the first char is always included
        assert_eq!(font(6).measure_line(b"abc", Some(1)), (6, 1));
    }

    #[test]
    fn word_wrap_in_place_basic() {
        // "hello world" with width-6, wrap at 36px:
        // "hello" = 30px fits, space + "world" = 6 + 30 = 36 — just over, so wrap
        let mut text = *b"hello world";
        font(6).word_wrap_in_place(&mut text, 36);
        assert_eq!(&text, b"hello\nworld");
    }

    #[test]
    fn word_wrap_in_place_no_wrap_needed() {
        let mut text = *b"hi there";
        let original = text;
        font(6).word_wrap_in_place(&mut text, 200);
        assert_eq!(text, original);
    }

    #[test]
    fn word_wrap_in_place_trailing_space() {
        // Trailing space has no following word, should not be converted
        let mut text = *b"hello ";
        font(6).word_wrap_in_place(&mut text, 10);
        assert_eq!(&text, b"hello ");
    }

    #[test]
    fn size_of_single_line() {
        // "abc" = 18px wide, 8px tall (one line)
        assert_eq!(font(6).size_of(b"abc", None), (18, 8));
    }

    #[test]
    fn size_of_multiline() {
        // "ab\ncd" = two lines of 12px, height = 16
        assert_eq!(font(6).size_of(b"ab\ncd", None), (12, 16));
    }

    #[test]
    fn size_of_empty() {
        assert_eq!(font(6).size_of(b"", None), (0, 0));
    }
}
