use super::*;

impl Lowerer {
    pub(super) fn resolve_struct_pattern_path(
        &mut self,
        path: &[ast::Ident],
        subject: TypeId,
        span: Span,
    ) -> Option<PatternTarget> {
        let name = match path {
            [] => return Some(PatternTarget::Struct(subject)),
            [name] => name,
            _ => {
                self.error(span, "invalid pattern path".into());
                return None;
            }
        };
        let matched = if let Some(target) = self.lexical_nested_nominal_target(&name.text) {
            self.nominal_target_for_type(subject) == Some(target)
        } else {
            use crate::imports::lookup::TypeLookupTarget;
            use crate::namespace::TopLevelTypeTarget;
            match self.resolve_type_lookup(name).ok()? {
                Some(TypeLookupTarget::Current(TopLevelTypeTarget::Nominal(target))) => {
                    if !self
                        .top_level_type_target_is_accessible(TopLevelTypeTarget::Nominal(target))
                    {
                        self.error(
                            name.span,
                            format!(
                                "type `{}` is not accessible from this source location",
                                name.text
                            ),
                        );
                        return None;
                    }
                    self.nominal_target_for_type(subject) == Some(target)
                }
                Some(TypeLookupTarget::Current(TopLevelTypeTarget::Alias(alias))) => {
                    let target = self.resolve_type_alias_id_reference(alias, name, false)?;
                    self.types_equal(target, subject)
                }
                Some(TypeLookupTarget::Dependency(binding)) => {
                    if let Some(owner) = binding.target().source_nominal() {
                        self.imported_nominal_owner(subject) == Some(owner)
                    } else {
                        let target =
                            self.resolve_imported_dependency_type_target(&binding, name, false)?;
                        self.types_equal(target, subject)
                    }
                }
                None => false,
            }
        };
        if matched {
            Some(PatternTarget::Struct(subject))
        } else {
            let found = self.type_name(subject);
            self.error(
                name.span,
                format!(
                    "pattern `{}` does not match a subject of type {found}",
                    name.text
                ),
            );
            None
        }
    }
}
