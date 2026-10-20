use super::*;
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

impl Lowerer {
    pub(super) fn documentation_imported_member(
        &mut self,
        receiver: hir::TypeId,
        candidate: hir::ImportedCallableDeclaration,
        property: bool,
    ) -> Relation {
        let declaration = candidate.interface();
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = declaration.owner() else {
            unreachable!("a member query returns nominal declarations")
        };
        let declaring = self
            .imported_member_owner_type(receiver, owner)
            .expect("an effective member retains its declaring application");
        let binders = declaration.type_parameters().binders();
        let mut bindings = self
            .imported_owner_arguments(declaring)
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                (
                    SignatureTypeKey::Binder {
                        depth: u32::from(!binders.is_empty()),
                        index: index as u32,
                    },
                    *ty,
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let offset = bindings.len();
        let ids = binders
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let id = self.fresh_type_param(offset + index);
                let ty = self.intern_type(hir::Type::Param(id));
                bindings.insert(
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    ty,
                );
                id
            })
            .collect::<Vec<_>>();
        let own = binders
            .iter()
            .zip(ids)
            .map(|(binder, id)| {
                self.resolve_imported_type_parameter(binder, id, &bindings, ast::Span::new(0, 0))
                    .expect("completed HIR has checked member parameter bounds")
            })
            .collect::<Vec<_>>();
        let mut parameters = self.type_params_in_scope.clone();
        parameters.extend(own.iter().cloned());
        let result = self
            .imported_signature_type_with_bindings(declaration.result(), &bindings)
            .expect("completed HIR has a checked member result");
        let result = self.type_name_with_params(result, &parameters);
        let (target, signature) = if property {
            let CallableTemplateOrigin::Accessor(accessor) = declaration.declaration() else {
                unreachable!("a getter query returns an accessor")
            };
            let property = self
                .dependencies
                .as_ref()
                .unwrap()
                .property_for_accessor(accessor)
                .expect("an accessor retains its logical property");
            let scoop_identity::PropertyOwner::Property(id) = property.declaration() else {
                unreachable!("a nominal property is not an extension")
            };
            (
                hir::AnnotationTargetV1::Property(id),
                format!(
                    "{} {}: {result}",
                    if property.accessors().setter().is_some() {
                        "var"
                    } else {
                        "val"
                    },
                    candidate.name()
                ),
            )
        } else {
            let values = declaration
                .parameters()
                .parameters()
                .iter()
                .zip(
                    candidate
                        .source_interface()
                        .expect("a method has its source parameter protocol")
                        .parameters()
                        .parameters(),
                )
                .map(|(parameter, source)| {
                    let mut ty = self
                        .imported_signature_type_with_bindings(parameter.value_type(), &bindings)
                        .expect("completed HIR has a checked member parameter");
                    let (prefix, suffix) = match source.calling() {
                        hir::CallableParameterCallingV1::Required => ("", ""),
                        hir::CallableParameterCallingV1::Default { .. } => ("", " = …"),
                        hir::CallableParameterCallingV1::VarargEmpty { .. }
                        | hir::CallableParameterCallingV1::VarargDefault { .. } => {
                            ty = self
                                .array_element_ty(ty)
                                .expect("a vararg parameter has an array type");
                            (
                                "vararg ",
                                if matches!(
                                    source.calling(),
                                    hir::CallableParameterCallingV1::VarargDefault { .. }
                                ) {
                                    " = …"
                                } else {
                                    ""
                                },
                            )
                        }
                    };
                    format!(
                        "{prefix}{}: {}{suffix}",
                        parameter.name().as_str(),
                        self.type_name_with_params(ty, &parameters)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let generics = crate::call_resolution::diagnostics::render_type_parameters(
                self,
                &own,
                &parameters,
            );
            let context = declaration
                .context_parameters()
                .iter()
                .map(|parameter| {
                    let ty = self
                        .imported_signature_type_with_bindings(parameter.value_type(), &bindings)
                        .expect("completed HIR has a checked context parameter");
                    self.type_name_with_params(ty, &parameters)
                })
                .collect::<Vec<_>>();
            let context = if context.is_empty() {
                String::new()
            } else {
                format!("context({}) ", context.join(", "))
            };
            let effects = declaration.effects();
            let effects = format!(
                "{}{}{}{}",
                if effects.safety() == hir::CallableSafetyV1::Unsafe {
                    "@Unsafe "
                } else {
                    ""
                },
                if effects.gc_effect() == scoop_identity::GcEffect::NoGc {
                    "@NoGC "
                } else {
                    ""
                },
                if effects.operator_role() != hir::CallableOperatorRoleV1::None {
                    "operator "
                } else {
                    ""
                },
                if effects.infix() == hir::CallableInfixV1::Infix {
                    "infix "
                } else {
                    ""
                }
            );
            (
                hir::AnnotationTargetV1::Callable(hir::AnnotationCallableV1::Declaration(
                    declaration.declaration(),
                )),
                format!(
                    "{context}{effects}{}fun {}{generics}({values}): {result}{}",
                    if declaration.effects().execution() == scoop_identity::Effect::Suspend {
                        "suspend "
                    } else {
                        ""
                    },
                    candidate.name(),
                    self.documentation_constraints(&own, &parameters)
                ),
            )
        };
        Relation {
            kind: RelationKind::Inherited,
            target: Symbol::Imported(target),
            signature,
            origin: self.type_name(declaring),
            visibility: match declaration.declared_visibility() {
                hir::DeclaredVisibilityV1::Public => hir::DeclaredVisibility::Public,
                hir::DeclaredVisibilityV1::Protected => hir::DeclaredVisibility::Protected,
                hir::DeclaredVisibilityV1::Internal => hir::DeclaredVisibility::Internal,
                hir::DeclaredVisibilityV1::Private => hir::DeclaredVisibility::Private,
            },
            hidden: self.documentation_imported_hidden(target)
                || self.documentation_imported_hidden(hir::AnnotationTargetV1::Nominal(owner)),
        }
    }

    pub(in crate::documentation) fn documentation_imported_hidden(
        &self,
        target: hir::AnnotationTargetV1,
    ) -> bool {
        self.dependencies.as_ref().is_some_and(|dependencies| {
            let hidden = |target| {
                dependencies.annotations(target).iter().any(|annotation| {
                    Some(annotation.annotation) == self.core_source_annotation("DocHidden")
                })
            };
            if hidden(target) {
                return true;
            }
            let hir::AnnotationTargetV1::Nominal(owner) = target else {
                return false;
            };
            dependencies
                .nominal_declaration(owner)
                .is_some_and(|nominal| {
                    nominal
                        .identity
                        .declaration()
                        .owners()
                        .owners()
                        .iter()
                        .any(|owner| {
                            use scoop_identity::{
                                DefinitionOwnerAtom as O, NominalDeclarationOwner as N,
                            };
                            match owner {
                                O::Type(id) => {
                                    hidden(hir::AnnotationTargetV1::Nominal(N::Concrete(*id)))
                                }
                                O::GenericType(id) => hidden(hir::AnnotationTargetV1::Nominal(
                                    N::GenericTemplate(*id),
                                )),
                                _ => false,
                            }
                        })
                })
        })
    }
}
