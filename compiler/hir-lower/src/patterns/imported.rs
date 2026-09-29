//! Dependency enum patterns use the same field normalization and coverage rules.

use super::*;

impl Lowerer {
    pub(super) fn resolve_imported_enum_pattern_path(
        &mut self,
        path: &[ast::Ident],
        owner: TypeId,
        span: Span,
    ) -> Option<PatternTarget> {
        let Some((name, qualifier)) = path.split_last() else {
            self.error(
                span,
                format!(
                    "a field pattern without a type name can only match a struct, found {}",
                    self.type_name(owner)
                ),
            );
            return None;
        };
        if !qualifier.is_empty() {
            let kind = match qualifier {
                [name] => ast::TypeRefKind::Named(name.clone()),
                _ => ast::TypeRefKind::Qualified {
                    path: qualifier.to_vec(),
                    arguments: Vec::new(),
                },
            };
            let reference = ast::TypeRef {
                kind,
                span: Span::new(qualifier[0].span.start, qualifier.last().unwrap().span.end),
            };
            let resolved = self.resolve_type_ref(&reference)?;
            if !self.types_equal(resolved, owner) {
                self.error(
                    span,
                    format!(
                        "pattern `{}` does not match a subject of type {}",
                        path.iter()
                            .map(|name| name.text.as_str())
                            .collect::<Vec<_>>()
                            .join("."),
                        self.type_name(owner)
                    ),
                );
                return None;
            }
        }
        let Type::ImportedEnum(enumeration) = &self.types[owner] else {
            unreachable!("dependency pattern resolution retains its enum subject")
        };
        let Some(variant) = enumeration
            .variants
            .iter()
            .position(|variant| variant.name == name.text)
        else {
            self.error(
                name.span,
                format!(
                    "enum `{}` has no variant `{}`",
                    enumeration.declaration.name(),
                    name.text
                ),
            );
            return None;
        };
        Some(PatternTarget::Variant(hir::EnumVariantApplication {
            owner,
            variant: enumeration.variants[variant].identity,
        }))
    }
}
