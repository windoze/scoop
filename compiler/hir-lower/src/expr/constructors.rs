use super::*;

mod arrays;
mod ffi;

mod classes;
mod imported_variants;
mod structs;
mod variants;

impl Lowerer {
    pub(in crate::expr) fn alias_fixed_expected(
        &mut self,
        alias: Option<&AliasExpansion>,
        supplied_type_arguments: &[ast::CallTypeArgument],
        contextual: Option<TypeId>,
    ) -> Option<Option<TypeId>> {
        let Some(alias) = alias else {
            return Some(contextual);
        };
        if !supplied_type_arguments.is_empty() {
            self.error(
                alias.name.span,
                format!("typealias `{}` is not generic", alias.name.text),
            );
            return None;
        }
        Some(Some(alias.target))
    }

    /// What a bare `Name` / `Name(...)` construction site resolves to.
    /// Call lowering probes a visible nominal constructor
    /// nominal transactionally before ordinary callable and contextual
    /// variant layers.
    pub(super) fn classify_constructor(&mut self, name: &ast::Ident) -> Option<Constructor> {
        if let Some(target) = self.lexical_nested_nominal_target(&name.text) {
            return Some(match target {
                crate::NominalTarget::Struct(struct_id) => {
                    let application = self.structs[struct_id].self_application;
                    Constructor::Struct {
                        struct_id,
                        ty: self.struct_applications[application].canonical_type,
                        alias: None,
                    }
                }
                crate::NominalTarget::Class(class_id) => Constructor::Class {
                    class_id,
                    alias: None,
                },
                crate::NominalTarget::Enum(_)
                | crate::NominalTarget::Interface(_)
                | crate::NominalTarget::Object(_) => Constructor::Unmatched,
            });
        }
        if self.source_type_alias_named(&name.text).is_some() {
            let target = self.resolve_type_alias_reference(name, false)?;
            let alias = AliasExpansion {
                name: name.clone(),
                target,
            };
            return Some(match self.nominal_target_for_type(target) {
                Some(crate::NominalTarget::Struct(struct_id)) => Constructor::Struct {
                    struct_id,
                    ty: target,
                    alias: Some(alias),
                },
                Some(crate::NominalTarget::Class(class_id)) => Constructor::Class {
                    class_id,
                    alias: Some(alias),
                },
                Some(crate::NominalTarget::Enum(_))
                | Some(crate::NominalTarget::Interface(_))
                | Some(crate::NominalTarget::Object(_))
                | None => Constructor::Unmatched,
            });
        }
        if let Some((struct_id, ty)) = self.top_level_struct_named(&name.text) {
            if !self.nominal_is_accessible(ty) {
                self.error(
                    name.span,
                    format!("struct `{}` is not accessible here", name.text),
                );
                return None;
            }
            return Some(Constructor::Struct {
                struct_id,
                ty,
                alias: None,
            });
        }
        if let Some((class_id, ty)) = self.top_level_class_named(&name.text) {
            if !self.nominal_is_accessible(ty) {
                self.error(
                    name.span,
                    format!("class `{}` is not accessible here", name.text),
                );
                return None;
            }
            return Some(Constructor::Class {
                class_id,
                alias: None,
            });
        }
        Some(Constructor::Unmatched)
    }
}
