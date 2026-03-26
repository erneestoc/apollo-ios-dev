//! TemplateString - indent-aware string builder.
//!
//! This is a faithful port of the Swift `TemplateString` type, which uses
//! StringInterpolation to provide indent-tracking and line-removal behavior.
//!
//! In Rust, we use explicit method calls instead of string interpolation.

/// An indent-aware string builder that replicates Swift's TemplateString behavior.
///
/// Key behaviors:
/// - `current_indent()`: scans backward from buffer position to find leading whitespace
/// - Multi-line strings are indented to match the current indent level
/// - `remove_line_if_empty()`: when an interpolated value is nil/empty, removes the current line
/// - `section`: empty sections also remove the preceding newline
/// - `last_line_was_removed`: when true, the next literal skips a leading `\n`
#[derive(Debug, Clone)]
pub struct TemplateString {
    value: String,
    last_line_was_removed: bool,
}

impl TemplateString {
    pub fn new(s: &str) -> Self {
        Self {
            value: s.to_string(),
            last_line_was_removed: false,
        }
    }

    pub fn from_builder(builder: Builder) -> Self {
        Self {
            value: builder.output(),
            last_line_was_removed: builder.last_line_was_removed,
        }
    }

    pub fn description(&self) -> &str {
        &self.value
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn into_string(self) -> String {
        self.value
    }
}

impl std::fmt::Display for TemplateString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl From<String> for TemplateString {
    fn from(s: String) -> Self {
        Self {
            value: s,
            last_line_was_removed: false,
        }
    }
}

impl From<&str> for TemplateString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Builder that constructs a TemplateString with indent-awareness.
///
/// This is the Rust equivalent of Swift's `TemplateString.StringInterpolation`.
pub struct Builder {
    pub(crate) last_line_was_removed: bool,
    buffer: String,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            last_line_was_removed: false,
            buffer: String::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            last_line_was_removed: false,
            buffer: String::with_capacity(capacity),
        }
    }

    /// Get the final output string.
    pub fn output(&self) -> String {
        if self.last_line_was_removed && self.buffer.ends_with('\n') {
            self.buffer[..self.buffer.len() - 1].to_string()
        } else {
            self.buffer.clone()
        }
    }

    /// Build into a TemplateString.
    pub fn build(self) -> TemplateString {
        TemplateString::from_builder(self)
    }

    // --- Literal ---

    /// Append a literal string (no indent processing).
    pub fn append_literal(&mut self, literal: &str) {
        if literal.is_empty() {
            return;
        }

        if self.last_line_was_removed && literal.starts_with('\n') {
            self.buffer.push_str(&literal[1..]);
        } else {
            self.buffer.push_str(literal);
        }
        self.last_line_was_removed = false;
    }

    // --- String interpolation (with indent) ---

    /// Append a string with indent-awareness.
    /// Multi-line strings have the current indent applied to each line.
    pub fn append_string(&mut self, string: &str) {
        let indent = self.current_indent();

        if indent.is_empty() {
            self.append_literal(string);
        } else {
            let parts: Vec<&str> = string.split('\n').collect();
            let indented = join_as_lines(&parts, &indent);
            self.append_literal(&indented);
        }
    }

    // --- Optional TemplateString ---

    /// Append an optional TemplateString. If None or empty, removes the current line.
    pub fn append_opt(&mut self, template: Option<&TemplateString>) {
        match template {
            Some(t) if !t.is_empty() => {
                self.append_string(t.description());
            }
            _ => {
                self.remove_line_if_empty();
            }
        }
    }

    /// Append a TemplateString (non-optional).
    pub fn append_template(&mut self, template: &TemplateString) {
        if template.is_empty() {
            self.remove_line_if_empty();
        } else {
            self.append_string(template.description());
        }
    }

    // --- Section ---

    /// Append a section. Empty sections remove the preceding newline.
    pub fn append_section(&mut self, section: &TemplateString) {
        self.append_template(section);
        if section.is_empty() && self.buffer.ends_with('\n') {
            self.buffer.pop();
        }
    }

    // --- If ---

    /// Conditional append. If false and no else, removes the current line.
    pub fn append_if(
        &mut self,
        condition: bool,
        then_template: &TemplateString,
        else_template: Option<&TemplateString>,
    ) {
        if condition {
            self.append_template(then_template);
        } else if let Some(else_t) = else_template {
            self.append_template(else_t);
        } else {
            self.remove_line_if_empty();
        }
    }

    /// Conditional string append.
    pub fn append_if_str(&mut self, condition: bool, then_str: &str) {
        if condition {
            self.append_string(then_str);
        } else {
            self.remove_line_if_empty();
        }
    }

    // --- If Let ---

    /// Append if value is Some. If None, removes the current line.
    pub fn append_if_let<T, F>(&mut self, opt: Option<T>, f: F)
    where
        F: FnOnce(T) -> TemplateString,
    {
        match opt {
            Some(val) => {
                let template = f(val);
                self.append_template(&template);
            }
            None => {
                self.remove_line_if_empty();
            }
        }
    }

    // --- For Each ---

    /// Append items from an iterator with a separator.
    /// If the iterator is empty, removes the current line.
    pub fn append_for_each<I, F>(
        &mut self,
        iter: I,
        separator: &str,
        terminator: Option<&str>,
        f: F,
    ) where
        I: IntoIterator,
        F: Fn(I::Item) -> Option<TemplateString>,
    {
        let mut result_string = String::new();

        for element in iter {
            if let Some(element_string) = f(element) {
                let desc = element_string.into_string();
                if result_string.is_empty() {
                    result_string = desc;
                } else {
                    result_string.push_str(separator);
                    result_string.push_str(&desc);
                }
            }
        }

        if result_string.is_empty() {
            self.remove_line_if_empty();
            return;
        }

        self.append_string(&result_string);
        if let Some(term) = terminator {
            self.append_string(term);
        }
    }

    /// Append a list with wrapping newlines if count > 1.
    pub fn append_list(&mut self, items: &[String], separator: &str) {
        let should_wrap = items.len() > 1;
        if should_wrap {
            self.append_string("\n  ");
        }
        self.append_for_each(
            items.iter(),
            separator,
            None,
            |item| Some(TemplateString::new(item)),
        );
        if should_wrap {
            self.append_string("\n");
        }
    }

    // --- Comment ---

    /// Append a comment (// prefix). If None/empty, removes the line.
    pub fn append_comment(&mut self, comment: Option<&str>) {
        self.append_comment_with_prefix(comment, "//");
    }

    /// Append a documentation comment (/// prefix). If None/empty, removes the line.
    pub fn append_documentation(&mut self, doc: Option<&str>) {
        self.append_comment_with_prefix(doc, "///");
    }

    fn append_comment_with_prefix(&mut self, comment: Option<&str>, prefix: &str) {
        match comment {
            Some(c) if !c.is_empty() => {
                let lines: Vec<&str> = c.split('\n').collect();
                let formatted = join_as_comment_lines(&lines, prefix);
                self.append_string(&formatted);
            }
            _ => {
                self.remove_line_if_empty();
            }
        }
    }

    // --- Helpers ---

    /// Get the current indentation by scanning backward from the buffer end.
    fn current_indent(&self) -> String {
        let bytes = self.buffer.as_bytes();
        let mut pos = bytes.len();

        // Find the start of the current line
        while pos > 0 && bytes[pos - 1] != b'\n' {
            pos -= 1;
        }

        // Extract leading whitespace
        let line_start = pos;
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
            pos += 1;
        }

        self.buffer[line_start..pos].to_string()
    }

    /// Remove the current line if it contains only whitespace.
    pub fn remove_line_if_empty(&mut self) {
        // Find the substring from the current position back to the start of the line
        let bytes = self.buffer.as_bytes();
        let mut count = 0;

        for &b in bytes.iter().rev() {
            if b == b'\n' {
                break;
            }
            count += 1;
        }

        // Check if all characters from start of line to current position are whitespace
        let line_content = &self.buffer[self.buffer.len() - count..];
        if line_content.chars().all(|c| c.is_whitespace() && c != '\n') {
            // Remove the whitespace
            self.buffer.truncate(self.buffer.len() - count);
            self.last_line_was_removed = true;
        }
    }
}

/// Join substrings as lines with indent applied to continuation lines.
fn join_as_lines(parts: &[&str], indent: &str) -> String {
    let mut result = String::new();
    let mut iter = parts.iter();

    if let Some(first) = iter.next() {
        result.push_str(first);
    }

    for next_line in iter {
        result.push('\n');
        if !next_line.is_empty() {
            result.push_str(indent);
            result.push_str(next_line);
        }
    }

    result
}

/// Join lines as comment lines with a prefix.
fn join_as_comment_lines(lines: &[&str], prefix: &str) -> String {
    let mut result = String::new();

    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        result.push_str(prefix);
        if !line.is_empty() {
            result.push(' ');
            result.push_str(line);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_literal() {
        let mut b = Builder::new();
        b.append_literal("hello world");
        assert_eq!(b.output(), "hello world");
    }

    #[test]
    fn test_multi_line_indentation() {
        let mut b = Builder::new();
        b.append_literal("  "); // 2 spaces of indent
        b.append_string("line1\nline2\nline3");
        assert_eq!(b.output(), "  line1\n  line2\n  line3");
    }

    #[test]
    fn test_remove_line_if_empty() {
        let mut b = Builder::new();
        b.append_literal("line1\n  ");
        b.append_opt(None);
        b.append_literal("\nline3");
        assert_eq!(b.output(), "line1\nline3");
    }

    #[test]
    fn test_section_removes_newline_when_empty() {
        // When a section is empty:
        // 1. remove_line_if_empty() sets last_line_was_removed = true
        // 2. Section removes trailing \n from buffer
        // 3. Next literal's leading \n is skipped due to last_line_was_removed
        // Result: the empty section + surrounding newlines collapse completely
        let mut b = Builder::new();
        b.append_literal("before\n");
        b.append_section(&TemplateString::new(""));
        b.append_literal("\nafter");
        assert_eq!(b.output(), "beforeafter");
    }

    #[test]
    fn test_section_with_content_preserves_newlines() {
        let mut b = Builder::new();
        b.append_literal("before\n");
        b.append_section(&TemplateString::new("middle"));
        b.append_literal("\nafter");
        assert_eq!(b.output(), "before\nmiddle\nafter");
    }

    #[test]
    fn test_if_true() {
        let mut b = Builder::new();
        b.append_literal("  ");
        b.append_if(true, &TemplateString::new("yes"), None);
        assert_eq!(b.output(), "  yes");
    }

    #[test]
    fn test_if_false_removes_line() {
        let mut b = Builder::new();
        b.append_literal("line1\n  ");
        b.append_if(false, &TemplateString::new("yes"), None);
        b.append_literal("\nline3");
        assert_eq!(b.output(), "line1\nline3");
    }

    #[test]
    fn test_for_each_with_separator() {
        let mut b = Builder::new();
        let items = vec!["a", "b", "c"];
        b.append_for_each(items.into_iter(), ",\n", None, |item| {
            Some(TemplateString::new(item))
        });
        assert_eq!(b.output(), "a,\nb,\nc");
    }

    #[test]
    fn test_for_each_empty_removes_line() {
        let mut b = Builder::new();
        b.append_literal("  ");
        let items: Vec<&str> = vec![];
        b.append_for_each(items.into_iter(), ",\n", None, |item| {
            Some(TemplateString::new(item))
        });
        assert_eq!(b.output(), "");
    }

    #[test]
    fn test_comment() {
        let mut b = Builder::new();
        b.append_comment(Some("This is a comment\nwith two lines"));
        assert_eq!(b.output(), "// This is a comment\n// with two lines");
    }

    #[test]
    fn test_documentation() {
        let mut b = Builder::new();
        b.append_documentation(Some("Doc comment"));
        assert_eq!(b.output(), "/// Doc comment");
    }

    #[test]
    fn test_none_comment_removes_line() {
        let mut b = Builder::new();
        b.append_literal("line1\n  ");
        b.append_comment(None);
        b.append_literal("\nline3");
        assert_eq!(b.output(), "line1\nline3");
    }

    #[test]
    fn test_indent_with_nested_multiline() {
        let mut b = Builder::new();
        b.append_literal("class Foo {\n  ");
        b.append_string("var a: Int\nvar b: String");
        b.append_literal("\n}");
        assert_eq!(
            b.output(),
            "class Foo {\n  var a: Int\n  var b: String\n}"
        );
    }

    #[test]
    fn test_last_line_was_removed_skips_next_newline() {
        let mut b = Builder::new();
        b.append_literal("line1\n  ");
        b.remove_line_if_empty();
        b.append_literal("\nline3"); // The \n should be skipped
        assert_eq!(b.output(), "line1\nline3");
    }

    #[test]
    fn test_generated_header() {
        let mut b = Builder::new();
        b.append_literal("// @generated\n// This file was automatically generated and should not be edited.\n");
        let result = b.output();
        assert!(result.starts_with("// @generated"));
    }
}
