use miette::{Diagnostic};
use thiserror::Error;

#[derive(Debug, Clone, Diagnostic, Error)]
#[error("Unexpected src '{text}'")]
pub struct SourceFile {
    #[source_code]
    pub text: String,
}

impl SourceFile {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
        }
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn slice(&self, start: usize, end: usize) -> &str {
        &self.text[start..end]
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let prefix = &self.text[..offset];

        let line =
            prefix
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count() + 1;
        let column = match prefix.rfind('\n') {
            Some(newline_offset) => { prefix[newline_offset + 1..].chars().count() + 1 }
            None => prefix.chars().count() + 1,
        };

        (line, column)
    }

    pub fn line_text(&self, line: usize) -> Option<&str> {
        if line == 0 {
            return None;
        }
        let mut current_line = 1;
        let mut line_start = 0;
        for (offset, character) in self.text.char_indices() {
            if character == '\n' {
                if current_line == line {
                    return Some(&self.text[line_start..offset]);
                }
                current_line += 1;
                line_start = offset + character.len_utf8();
            }
        }
        if current_line == line {
            return Some(&self.text[line_start..]);
        }
        None
    }

    pub fn line_start(&self, line: usize) -> Option<usize> {
        if line == 0 {
            return None;
        }
        if line == 1 {
            return Some(0);
        }
        let mut current_line = 1;
        for (offset, character) in self.text.char_indices() {
            if character == '\n' {
                current_line += 1;
                if current_line == line {
                    return Some(offset + character.len_utf8());
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_first_line_position() {
        let source = SourceFile::new("abc");

        assert_eq!(source.line_col(0), (1, 1));
        assert_eq!(source.line_col(1), (1, 2));
        assert_eq!(source.line_col(3), (1, 4));
    }

    #[test]
    fn calculates_positions_after_newline() {
        let source = SourceFile::new("abc\ndef");

        assert_eq!(source.line_col(4), (2, 1));
        assert_eq!(source.line_col(5), (2, 2));
        assert_eq!(source.line_col(7), (2, 4));
    }

    #[test]
    fn returns_source_lines() {
        let source = SourceFile::new("one\ntwo\nthree");

        assert_eq!(source.line_text(1), Some("one"));
        assert_eq!(source.line_text(2), Some("two"));
        assert_eq!(source.line_text(3), Some("three"));
        assert_eq!(source.line_text(4), None);
    }

    #[test]
    fn calculates_line_start() {
        let source = SourceFile::new("one\ntwo\nthree");

        assert_eq!(source.line_start(1), Some(0));
        assert_eq!(source.line_start(2), Some(4));
        assert_eq!(source.line_start(3), Some(8));
        assert_eq!(source.line_start(4), None);
    }
}

