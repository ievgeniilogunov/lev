use std::fmt;

use crate::compiler::source::SourceFile;

use super::source::Span;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub span: Option<Span>,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    pub fn at(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }

    pub fn render(&self, source: &SourceFile, filename: &str) -> String {
        let Some(span) = self.span else {
            return format!("error: {}", self.message);
        };
        let (line, column) = source.line_col(span.start);

        let source_line = source.line_text(line).unwrap_or("");

        let line_number_width = line.to_string().len();

        let line_start = source.line_start(line).unwrap_or(span.start);

        let span_start_on_line = span.start.saturating_sub(line_start);
        let span_end_on_line = span.end.saturating_sub(line_start);

        let source_line_byte_length = source_line.len();

        let start = span_start_on_line.min(source_line_byte_length);
        let mut end = span_end_on_line.min(source_line_byte_length);

        if end <= start {
            end = start.saturating_add(1).min(source_line_byte_length);
        }

        let caret_length = if end > start {
            source_line[start..end].chars().count().max(1)
        } else {
            1
        };

        let indentation = source_line[..start]
            .chars()
            .map(|character| if character == '\t' { '\t' } else { ' ' })
            .collect::<String>();

        let carets = "^".repeat(caret_length);

        format!(
            "error: {}\n --> {}:{}:{}\n {:>width$} |\n {:>width$} | {}\n {:>width$} | {}{}\n",
            self.message,
            filename,
            line,
            column,
            "",
            line,
            source_line,
            "",
            indentation,
            carets,
            width = line_number_width
        )
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.span {
            Some(span) => { write!(f, "{} at {}..{}", self.message, span.start, span.end) }

            None => { write!(f, "{}", self.message) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::source::{SourceFile, Span};

    #[test]
    fn renders_diagnostic_with_line_and_column() {
        let source = SourceFile::new(
            "fn main(): int {\n\
             return missing;\n\
             }\n",
        );

        let start = source.text.find("missing").unwrap();
        let end = start + "missing".len();

        let diagnostic = Diagnostic::at(
            "unknown variable 'missing'",
            Span::new(start, end),
        );

        let rendered = diagnostic.render(&source, "test.lev");

        assert!(rendered.contains("test.lev:2:8"));
        assert!(rendered.contains("return missing;"));
        assert!(rendered.contains("^^^^^^^"));
        assert!(rendered.contains("unknown variable 'missing'"));
    }

    #[test]
    fn renders_diagnostic_without_span() {
        let source = SourceFile::new("fn main(): int {}");

        let diagnostic = Diagnostic::new("internal compiler error");

        let rendered = diagnostic.render(&source, "test.lev");

        assert_eq!(rendered, "error: internal compiler error");
    }
}
