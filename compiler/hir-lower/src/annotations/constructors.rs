use super::*;

impl Lowerer {
    pub(crate) fn constructor_safety(
        &mut self,
        annotations: &[ast::Annotation],
        span: ast::Span,
    ) -> hir::Safety {
        let mut safety = hir::Safety::Safe;
        let mut seen = HashSet::new();
        for annotation in annotations
            .iter()
            .filter(|annotation| matches!(annotation.name.text.as_str(), "Safe" | "Unsafe"))
        {
            let name = annotation.name.text.as_str();
            if !seen.insert(name) {
                self.error(
                    annotation.span,
                    format!("annotation `@{name}` must not be repeated"),
                );
                continue;
            }
            if self.annotation_marker(annotation) {
                safety = if name == "Unsafe" {
                    hir::Safety::Unsafe
                } else {
                    hir::Safety::Safe
                };
            }
        }
        if seen.contains("Safe") && seen.contains("Unsafe") {
            self.error(span, "`@Safe` and `@Unsafe` cannot be combined".to_string());
        }
        safety
    }
}
