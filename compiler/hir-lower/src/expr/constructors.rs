use super::*;

mod arrays;
mod ffi;

mod classes;
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

    /// What a `Name` / `Name(...)` construction site resolves to. A
    /// dotted path `E.V` is always an enum variant; a bare name is a
    /// globally visible `Option` variant (`Some` / `None`), then a
    /// struct, then a class, then — for `Call` nodes only — a
    /// function.
    pub(super) fn classify_constructor(&mut self, name: &ast::Ident) -> Option<Constructor> {
        if let Some((enum_name, variant_name)) = name.text.split_once('.') {
            let lexical_target = self.lexical_nested_nominal_target(enum_name);
            let (enum_id, alias) = if let Some(target) = lexical_target {
                let crate::NominalTarget::Enum(enum_id) = target else {
                    self.error(
                        name.span,
                        format!("type `{enum_name}` does not name an enum"),
                    );
                    return None;
                };
                (enum_id, None)
            } else if self.source_type_alias_named(enum_name).is_some() {
                let enum_name = ast::Ident {
                    text: enum_name.to_string(),
                    span: Span::new(
                        name.span.start,
                        name.span.start
                            + u32::try_from(enum_name.len())
                                .expect("an identifier span length fits in u32"),
                    ),
                };
                let target = self.resolve_type_alias_reference(&enum_name, false)?;
                let Type::Enum(application) = self.types[target] else {
                    self.error(
                        enum_name.span,
                        format!("typealias `{}` does not name an enum", enum_name.text),
                    );
                    return None;
                };
                (
                    self.enum_applications[application].template,
                    Some(AliasExpansion {
                        name: enum_name,
                        target,
                    }),
                )
            } else {
                let Some(&enum_id) = self.enums_by_name.get(enum_name) else {
                    self.error(name.span, format!("unknown enum `{enum_name}`"));
                    return None;
                };
                (enum_id, None)
            };
            if !self.enum_is_accessible(enum_id) {
                self.error(
                    name.span,
                    format!("enum `{enum_name}` is not accessible here"),
                );
                return None;
            }
            let Some(variant) = self.find_variant(enum_id, variant_name) else {
                self.error(
                    name.span,
                    format!("enum `{enum_name}` has no variant `{variant_name}`"),
                );
                return None;
            };
            return Some(Constructor::Variant {
                enum_id,
                variant,
                alias,
            });
        }
        if let Some((enum_id, variant)) = self.option_variant(&name.text) {
            return Some(Constructor::Variant {
                enum_id,
                variant,
                alias: None,
            });
        }
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
        if let Some(&(struct_id, ty)) = self.structs_by_name.get(&name.text) {
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
        if let Some(&(class_id, ty)) = self.classes_by_name.get(&name.text) {
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
