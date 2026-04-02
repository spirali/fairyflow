use crate::Color;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Pre-computed foreground colors for every byte position in a text block.
/// Produced by `highlight_text` and queried at render time per glyph.
pub struct SyntaxColors {
    /// Sorted list of `(start_byte, rgba8)` pairs where each entry is valid
    /// from `start_byte` up to (but not including) the next entry's start.
    tokens: Vec<(usize, [u8; 4])>,
    text_len: usize,
}

impl SyntaxColors {
    /// Returns the SH foreground color for byte position `pos` in the source text,
    /// or `None` if `pos` is out of range.
    pub fn color_at(&self, pos: usize) -> Option<Color> {
        if pos >= self.text_len || self.tokens.is_empty() {
            return None;
        }
        // Last token that starts at or before `pos`.
        let idx = self.tokens.partition_point(|(start, _)| *start <= pos);
        if idx == 0 {
            return None;
        }
        let [r, g, b, a] = self.tokens[idx - 1].1;
        Some(Color::from_rgba8(r, g, b, a))
    }
}

/// Run syntect syntax highlighting on `text` for the given language and theme.
///
/// `language` is matched against syntax names and extensions (case-insensitive).
/// If no match is found, plain-text highlighting is used (no colouring).
/// `theme_name` falls back to `"InspiredGitHub"` when not found.
pub fn highlight_text(
    text: &str,
    language: &str,
    theme_name: &str,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
) -> SyntaxColors {
    let syntax = syntax_set
        .find_syntax_by_name(language)
        .or_else(|| syntax_set.find_syntax_by_extension(language))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let theme = theme_set
        .themes
        .get(theme_name)
        .or_else(|| theme_set.themes.get("InspiredGitHub"))
        .or_else(|| theme_set.themes.values().next())
        .expect("no syntect themes available");

    let mut h = HighlightLines::new(syntax, theme);
    let mut tokens: Vec<(usize, [u8; 4])> = Vec::new();
    let mut offset = 0usize;

    for line in LinesWithEndings::from(text) {
        match h.highlight_line(line, syntax_set) {
            Ok(ranges) => {
                for (style, token_str) in &ranges {
                    let c = style.foreground;
                    tokens.push((offset, [c.r, c.g, c.b, c.a]));
                    offset += token_str.len();
                }
            }
            Err(_) => {
                // On parse error, skip coloring for this line.
                offset += line.len();
            }
        }
    }

    SyntaxColors {
        tokens,
        text_len: text.len(),
    }
}
