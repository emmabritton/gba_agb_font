pub const GLYPH_COUNT_FULL: usize = 256;
pub const HEADER_ALL: usize = 256 + 1 + 1 + 1 + 1;

/// 4bpp font for gba covering all 256 Latin-1 code points.
/// Font data location (ROM/IWRAM/EWRAM) is determined by the `full_font!` macro.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FullFont {
    pub char_widths: [u8; GLYPH_COUNT_FULL],
    pub data: &'static [u32],
    pub glyph_height: u32,
    pub glyph_size: usize,
    pub row_u32s: usize,
}

/// Construct a static [`FullFont`] from a byte array literal, with optional `iwram` or `ewram` placement.
#[macro_export]
macro_rules! full_font {
    ($bytes:expr) => {{
        #[repr(C, align(4))]
        struct AlignedFont([u8; { $bytes.len() }]);
        static FONT_BYTES: AlignedFont = AlignedFont(*$bytes);
        $crate::font::full::FullFont::from_static_bytes(&FONT_BYTES.0)
    }};
    ($bytes:expr, iwram) => {{
        #[repr(C, align(4))]
        struct AlignedFont([u8; { $bytes.len() }]);
        #[unsafe(link_section = ".iwram")]
        static FONT_BYTES: AlignedFont = AlignedFont(*$bytes);
        $crate::font::full::FullFont::from_static_bytes(&FONT_BYTES.0)
    }};
    ($bytes:expr, ewram) => {{
        #[repr(C, align(4))]
        struct AlignedFont([u8; { $bytes.len() }]);
        #[unsafe(link_section = ".ewram")]
        static FONT_BYTES: AlignedFont = AlignedFont(*$bytes);
        $crate::font::full::FullFont::from_static_bytes(&FONT_BYTES.0)
    }};
}

impl FullFont {
    /// Parse a mode-1 binary font blob. Panics if the data is too short or has the wrong mode byte.
    pub const fn from_static_bytes(bytes: &'static [u8]) -> Self {
        assert!(bytes.len() >= HEADER_ALL, "font bytes too short");
        let mode = bytes[0];
        assert!(mode == 1, "invalid font mode (must be 1 for full font)");

        let glyph_width = bytes[1];
        let glyph_height = bytes[2] as u32;
        let row_u32s = (glyph_width as usize + 7) >> 3;

        assert!(
            bytes.len() >= 3 + GLYPH_COUNT_FULL,
            "font bytes too short for full-256 mode"
        );
        let mut char_widths = [0u8; GLYPH_COUNT_FULL];
        let mut i = 0usize;
        while i < GLYPH_COUNT_FULL {
            char_widths[i] = bytes[3 + i];
            i += 1;
        }

        let data_len = bytes.len() - HEADER_ALL;
        assert!(
            data_len.is_multiple_of(4),
            "font pixel data length is not a multiple of 4"
        );
        let data: &'static [u32] = unsafe {
            core::slice::from_raw_parts(bytes.as_ptr().add(HEADER_ALL) as *const u32, data_len / 4)
        };

        let glyph_size = row_u32s * glyph_height as usize;
        Self {
            glyph_height,
            glyph_size,
            row_u32s,
            char_widths,
            data,
        }
    }
}
