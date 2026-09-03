use super::*;

impl Lowerer {
    /// `scoop.core` must define exactly one enum named `Option` with
    /// exactly one type parameter (hir docs, spec 7.2). A second
    /// `Option` was already rejected as a duplicate enum in pass 1, so
    /// at most one candidate reaches here.
    pub(crate) fn validate_option_enum(&mut self, files: &[ast::SourceFile]) {
        let Some(&(id, file_index, span, type_param_count)) = self.option_candidates.first() else {
            // Attribute to the first file: with a core library present
            // that is a core file; without one it is the user file.
            self.current_file = 0;
            self.error(
                files[0].span,
                "scoop.core must define an enum `Option<T>`".to_string(),
            );
            return;
        };
        if type_param_count != 1 {
            self.current_file = file_index;
            self.error(
                span,
                format!(
                    "enum `Option` in scoop.core must have exactly one type parameter, found {type_param_count}"
                ),
            );
            return;
        }
        self.option_enum = Some(id);
    }
}
