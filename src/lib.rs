#![no_std]

use crate::full::FullFont;
use crate::printable::PrintableFont;

pub mod full;
pub mod printable;

/// 4bpp bitmap font for the GBA, loaded from a binary font blob.
pub trait AgbFont {
    /// Advance widths in pixels for each character in the font's range.
    fn char_widths(&self) -> &[u8];

    /// Advance width in pixels for character `c`.
    #[inline]
    fn char_width(&self, c: u8) -> u8 {
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
        if cfg!(debug_assertions) && self.char_offset() != 0 && !(32..=126).contains(&c) {
            panic!("glyph {c} out of printable bounds");
        }
        let idx = c as usize - self.char_offset();
        let offset = idx * self.glyph_size();
        unsafe {
            self.data()
                .get_unchecked(offset..offset + self.glyph_size())
        }
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

    /// Preprocess `text` for word-aware wrapping by replacing spaces with `\n` where the next
    /// word would overflow `wrap_px`. Words longer than `wrap_px` are left for the renderers
    /// character level hard break to handle.
    fn word_wrap(&self, text: &mut [u8], wrap_px: u32) {
        let mut line_w: u32 = 0;
        let mut i = 0;
        while i < text.len() {
            let c = text[i];
            if c == b'\n' {
                line_w = 0;
            } else if c == b' ' {
                let space_w = self.char_width(b' ') as u32;
                let next_start = i + 1;
                let next_end = text[next_start..]
                    .iter()
                    .position(|&x| x == b' ' || x == b'\n')
                    .map(|p| next_start + p)
                    .unwrap_or(text.len());
                let next_w: u32 = text[next_start..next_end]
                    .iter()
                    .map(|&x| self.char_width(x) as u32)
                    .sum();
                if next_end > next_start && line_w > 0 && line_w + space_w + next_w > wrap_px {
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

    /// Returns the `(width, height)` in pixels required to render `text`, with optional line-wrapping.
    fn size_of(&self, text: &[u8], wrap_at: Option<u8>) -> (u8, u8) {
        if text.is_empty() {
            return (0, 0);
        }
        let max_width = wrap_at.map(|v| v as u32);
        let mut max_w: u32 = 0;
        let mut total_h: u32 = 0;
        let mut remaining = text;
        loop {
            let (line_w, consumed) = self.measure_line(remaining, max_width);
            if line_w > max_w {
                max_w = line_w;
            }
            total_h += self.glyph_height();
            remaining = &remaining[consumed..];
            if remaining.is_empty() {
                break;
            }
        }
        (max_w as u8, total_h as u8)
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
