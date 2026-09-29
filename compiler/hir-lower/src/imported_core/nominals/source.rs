//! Source type arguments use the dependency declaration's actual bounds.

use super::*;

impl Lowerer {
    pub(crate) fn resolve_imported_nominal_type_arguments(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        name: &scoop_ast::Ident,
        arguments: &[scoop_ast::TypeRef],
    ) -> Option<hir::TypeId> {
        let hir::ImportedTarget::GenericType(identity) = binding.target() else {
            return self.resolve_imported_dependency_type_target(
                binding,
                name,
                !arguments.is_empty(),
            );
        };
        let owner = hir::SourceNominalId::GenericTemplate(identity.persistent());
        self.resolve_imported_nominal_owner_arguments(owner, name, arguments)
    }

    pub(crate) fn resolve_imported_nominal_owner_arguments(
        &mut self,
        owner: hir::SourceNominalId,
        name: &scoop_ast::Ident,
        arguments: &[scoop_ast::TypeRef],
    ) -> Option<hir::TypeId> {
        let declaration = self
            .dependencies
            .as_ref()?
            .nominal_declaration(owner)?
            .clone();
        let parameters = declaration.interface.type_parameters().binders();
        if arguments.len() != parameters.len() {
            self.error(
                name.span,
                format!(
                    "type `{}` takes {} type argument(s), but {} were supplied",
                    name.text,
                    parameters.len(),
                    arguments.len()
                ),
            );
            return None;
        }
        let argument_types = arguments
            .iter()
            .map(|argument| self.resolve_type_ref(argument))
            .collect::<Option<Vec<_>>>()?;
        let bindings = argument_types
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                (
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    *ty,
                )
            })
            .collect();
        let mut valid = true;
        for ((parameter, argument), syntax) in parameters.iter().zip(&argument_types).zip(arguments)
        {
            let kind = match parameter.bounds() {
                hir::TypeParameterBoundsV1::Value => hir::TypeParamKind::Value,
                hir::TypeParameterBoundsV1::Ref => hir::TypeParamKind::Ref,
                hir::TypeParameterBoundsV1::Unconstrained
                | hir::TypeParameterBoundsV1::Nominal(_) => hir::TypeParamKind::Any,
            };
            if !self.type_satisfies_kind(*argument, kind) {
                self.error(
                    syntax.span,
                    format!(
                        "type argument `{}` for `{}` of type `{}` must satisfy `{}`",
                        self.type_name(*argument),
                        parameter.name().as_str(),
                        name.text,
                        if kind == hir::TypeParamKind::Value {
                            "value"
                        } else {
                            "ref"
                        }
                    ),
                );
                valid = false;
            }
            if let hir::TypeParameterBoundsV1::Nominal(bounds) = parameter.bounds() {
                for bound in bounds
                    .class()
                    .into_iter()
                    .chain(bounds.interfaces().values())
                {
                    let required =
                        match self.imported_signature_type_with_bindings(bound, &bindings) {
                            Ok(required) => required,
                            Err(error) => {
                                self.error(
                                    syntax.span,
                                    format!("invalid dependency type bound: {error:?}"),
                                );
                                return None;
                            }
                        };
                    if !self.is_subtype(*argument, required) {
                        self.error(syntax.span, format!("type argument `{}` for `{}` of type `{}` must satisfy upper bound `{}`", self.type_name(*argument), parameter.name().as_str(), name.text, self.type_name(required)));
                        valid = false;
                    }
                }
            }
        }
        if !valid {
            return None;
        }
        let ty = match self.imported_nominal_application(owner, argument_types) {
            Ok(ty) => ty,
            Err(error) => {
                self.error(
                    name.span,
                    format!("cannot resolve dependency type `{}`: {error:?}", name.text),
                );
                return None;
            }
        };
        Some(ty)
    }
}
