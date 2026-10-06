//! Pattern prefixes use ordinary declaration lookup before matching applications.

use super::*;
use crate::types::ResolvedTypeName;

impl Lowerer {
    pub(super) fn resolve_enum_pattern_path(
        &mut self,
        path: &[ast::Ident],
        subject: TypeId,
        span: Span,
    ) -> Option<PatternTarget> {
        let Some((name, qualifier)) = path.split_last() else {
            self.error(
                span,
                format!(
                    "a field pattern without a type name can only match a struct, found {}",
                    self.type_name(subject),
                ),
            );
            return None;
        };
        if !qualifier.is_empty() && !self.pattern_type_name_matches(qualifier, subject)? {
            self.pattern_type_mismatch(path, subject, span);
            return None;
        }
        let Some(variant) = self.named_enum_variant(subject, &name.text) else {
            let owner = self
                .nominal_application(subject)
                .expect("an enum pattern retains its nominal application");
            let owner_name = self.nominal_template_name(owner.template);
            self.error(
                name.span,
                format!("enum `{owner_name}` has no variant `{}`", name.text,),
            );
            return None;
        };
        Some(PatternTarget::Variant(variant))
    }

    pub(super) fn pattern_type_name_matches(
        &mut self,
        path: &[ast::Ident],
        subject: TypeId,
    ) -> Option<bool> {
        let matched = match self.resolve_type_name_path(path, false).ok()? {
            Some(ResolvedTypeName::Nominal(owner)) => self
                .nominal_application(subject)
                .is_some_and(|subject| subject.template == owner),
            Some(ResolvedTypeName::Applied(ty)) => self.types_equal(ty, subject),
            Some(ResolvedTypeName::Annotation(_)) | None => false,
        };
        Some(matched)
    }

    pub(super) fn pattern_type_mismatch(
        &mut self,
        path: &[ast::Ident],
        subject: TypeId,
        span: Span,
    ) {
        let name = path
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        let span = if path.len() == 1 { path[0].span } else { span };
        self.error(
            span,
            format!(
                "pattern `{name}` does not match a subject of type {}",
                self.type_name(subject),
            ),
        );
    }
}
