use super::path_service::{PARENT_ROW, PathState};

pub(crate) struct Navigator<'a> {
    pub(crate) path_state: &'a PathState,
}

impl<'a> Navigator<'a> {
    pub(crate) fn new(path_state: &'a PathState) -> Self {
        Self { path_state }
    }

    pub(crate) fn completed_path(&self, highlighted_row: &str) -> String {
        if highlighted_row == PARENT_ROW {
            return self.path_state.parent_display_path.clone();
        }
        format!(
            "{}{highlighted_row}/",
            self.path_state.completion_display_prefix
        )
    }

    pub(crate) fn ghost_text(&self, highlighted_row: Option<&str>) -> String {
        let Some(row) = highlighted_row.filter(|row| *row != PARENT_ROW) else {
            return String::new();
        };
        let input = &self.path_state.input;
        let completed = self.completed_path(row);
        if let Some(rest) = completed.strip_prefix(input.as_str()) {
            return rest.to_owned();
        }

        let trimmed = input.trim();
        if trimmed.contains('/') || trimmed.starts_with('~') {
            return String::new();
        }
        let trimmed_lower = trimmed.to_lowercase();
        let row_lower = row.to_lowercase();
        if row_lower == trimmed_lower {
            return "/".to_owned();
        }
        if !row_lower.starts_with(&trimmed_lower) {
            return String::new();
        }
        format!(
            "{}/",
            split_after_case_insensitive_prefix(row, &trimmed_lower)
        )
    }
}

/// Returns the suffix of `row` after a case-insensitive prefix whose
/// lowercased form is `prefix_lower`.
///
/// The cut is computed by walking `row`'s own character boundaries and
/// comparing in lowercased space, because lowercasing can change byte
/// lengths (`K` U+212A → `k`, `İ` U+0130 → `i` + combining dot): slicing
/// `row` at the typed prefix's byte length can land mid-character or out
/// of bounds and panic during rendering.
fn split_after_case_insensitive_prefix<'a>(row: &'a str, prefix_lower: &str) -> &'a str {
    let mut consumed = 0;
    for (index, character) in row.char_indices() {
        if consumed >= prefix_lower.len() {
            return &row[index..];
        }
        consumed += character.to_lowercase().collect::<String>().len();
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::super::path_service::PathState;
    use super::Navigator;

    fn state(input: &str, prefix: &str) -> PathState {
        PathState {
            input: input.to_owned(),
            directory_path: String::new(),
            leaf_filter: String::new(),
            confirm_path: String::new(),
            standardized_confirm_path: String::new(),
            parent_display_path: String::new(),
            completion_display_prefix: prefix.to_owned(),
        }
    }

    fn ghost(input: &str, row: &str) -> String {
        let state = state(input, "/tmp/");
        Navigator::new(&state).ghost_text(Some(row))
    }

    #[test]
    fn ascii_prefix_ghost_is_unchanged() {
        assert_eq!(ghost("NGM", "ngm-agentic-harness"), "-agentic-harness/");
        assert_eq!(ghost("Ab", "abc"), "c/");
        assert_eq!(ghost("nomatch", "ngm-agentic-harness"), "");
    }

    #[test]
    fn kelvin_sign_prefix_does_not_slice_mid_char() {
        // '\u{212a}' (Kelvin sign) lowercases to 'k': the lowered prefix
        // matches, but the original byte lengths differ.
        assert_eq!(ghost("k", "\u{212a}elvin"), "elvin/");
    }

    #[test]
    fn capital_sharp_s_prefix_does_not_slice_out_of_bounds() {
        // '\u{1e9e}' (capital sharp S) lowercases to 'ß', so the typed
        // prefix is longer in bytes than the row it matches.
        assert_eq!(ghost("\u{1e9e}pecial", "ßpecial"), "/");
    }

    #[test]
    fn precomposed_prefix_over_decomposed_row_does_not_slice_mid_char() {
        // Row in NFD ('i' + combining dot) vs typed precomposed 'İ'.
        assert_eq!(ghost("\u{130}", "i\u{307}x"), "x/");
    }

    #[test]
    fn unicode_case_equality_shows_slash() {
        // Full-equality check uses Unicode lowercase like the prefix
        // check: Kelvin-sign row vs typed 'k' is a full match, not a prefix.
        assert_eq!(ghost("k", "\u{212a}"), "/");
    }
}
