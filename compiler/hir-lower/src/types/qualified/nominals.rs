//! Type arguments for nested source nominal declarations.

use super::*;

impl Lowerer {
    pub(in crate::types) fn resolve_nested_nominal_application(
        &mut self,
        target: NominalTarget,
        arguments: &[ast::TypeRef],
        span: ast::Span,
        display_name: &str,
    ) -> Option<TypeId> {
        self.require_editing_type(
            crate::editing::types::TypeTarget::Nominal(
                self.nominal_identity(target.owner()).declaration_id(),
            ),
            span,
        )?;
        let (kind, params) = match target {
            NominalTarget::Struct(id) => ("struct", self.structs[id].type_params.clone()),
            NominalTarget::Enum(id) => ("enum", self.enums[id].type_params.clone()),
            NominalTarget::Class(id) => ("class", self.classes[id].type_params.clone()),
            NominalTarget::Interface(id) => ("interface", self.interfaces[id].type_params.clone()),
            NominalTarget::Object(id) => {
                if !self.classes[self.objects[id].backing_class]
                    .type_params
                    .is_empty()
                {
                    self.error(
                        span,
                        "generic companion type requires complete host type arguments".into(),
                    );
                    return None;
                }
                ("object", Vec::new())
            }
        };
        let prefix = self
            .local_type_parameter_count(target.owner())
            .max(match target {
                NominalTarget::Class(id) => self.enclosing_parameter_count(id),
                _ => 0,
            });
        if !params[..prefix].iter().all(|parameter| {
            self.type_params_in_scope
                .iter()
                .any(|visible| visible.id == parameter.id)
        }) {
            self.error(
                span,
                "inner type requires complete outer type arguments".into(),
            );
            return None;
        }
        let arity = params.len() - prefix;
        if arity == 0 && !arguments.is_empty() {
            self.error(span, format!("{kind} `{display_name}` is not generic"));
            return None;
        }
        if arity != arguments.len() {
            self.error(
                span,
                if arguments.is_empty() {
                    format!("generic {kind} `{display_name}` requires {arity} type argument(s)")
                } else {
                    format!(
                        "{kind} `{display_name}` takes {arity} type argument(s), but {} were supplied",
                        arguments.len()
                    )
                },
            );
            return None;
        }
        let mut resolved = params[..prefix]
            .iter()
            .map(|parameter| self.intern_type(Type::Param(parameter.id)))
            .collect::<Vec<_>>();
        for argument in arguments {
            resolved.push(self.resolve_type_ref(argument)?);
        }
        if !self.check_type_argument_kinds(
            &params,
            &resolved,
            span,
            &format!("{kind} `{display_name}`"),
        ) {
            return None;
        }
        Some(match target {
            NominalTarget::Struct(id) => self.struct_application(id, resolved),
            NominalTarget::Enum(id) => self.enum_application(id, resolved),
            NominalTarget::Class(id) => self.class_application(id, resolved),
            NominalTarget::Interface(id) => self.source_interface_type(id, resolved),
            NominalTarget::Object(id) => {
                self.object_types[self.objects[id].object_type].canonical_type
            }
        })
    }
}
