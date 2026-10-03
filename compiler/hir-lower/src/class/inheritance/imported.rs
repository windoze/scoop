//! Class inheritance uses the same dependency declarations as member calls.

use super::interfaces::InterfaceSignature;
use super::*;
use hir::ImportedCallableSource;

#[derive(Clone)]
pub(super) struct ImportedInheritedMethod {
    pub owner: TypeId,
    pub declaration: hir::ImportedCallableDeclaration,
    pub signature: InterfaceSignature,
}

impl Lowerer {
    pub(super) fn inherited_imported_class_methods(
        &mut self,
        class: ClassId,
        name: &str,
        span: ast::Span,
    ) -> Vec<ImportedInheritedMethod> {
        let lookup = if let Some(name) = name.strip_prefix("$get$") {
            hir::ImportedMemberLookup::PropertyGetter(name)
        } else if let Some(name) = name.strip_prefix("$set$") {
            hir::ImportedMemberLookup::PropertySetter(name)
        } else {
            hir::ImportedMemberLookup::Name(name)
        };
        let mut current = self.classes[class].base_class;
        let mut seen = std::collections::HashSet::new();
        let mut methods = Vec::<ImportedInheritedMethod>::new();
        while let Some(ty) = current {
            if !seen.insert(ty) {
                break;
            }
            let Type::Class(application) = self.types[ty] else {
                unreachable!("class inheritance follows class types")
            };
            let application = self.class_applications[application].clone();
            current = self
                .class_definition(application.template)
                .base_class
                .map(|base| self.instantiate_ty(base, &application.arguments));
            if self.source_class_id(application.template).is_some() {
                continue;
            }
            let candidates = self
                .dependencies
                .as_ref()
                .expect("dependency class has a catalog")
                .member_callable_candidates(application.template, lookup);
            let candidates = match candidates {
                Ok(candidates) => candidates,
                Err(error) => {
                    self.error(
                        span,
                        format!("invalid inherited dependency member: {error}"),
                    );
                    return methods;
                }
            };
            for declaration in candidates {
                let callable = declaration.interface();
                if !self.imported_callable_is_accessible(callable, None)
                    || !callable.type_parameters().is_empty()
                {
                    continue;
                }
                let signature =
                    self.imported_inheritance_signature(name, callable, &application.arguments);
                let Some(signature) = signature else {
                    self.error(span, format!("invalid inherited signature for `{name}`"));
                    continue;
                };
                if methods
                    .iter()
                    .any(|method| self.same_interface_signature(&method.signature, &signature))
                {
                    continue;
                }
                methods.push(ImportedInheritedMethod {
                    owner: ty,
                    declaration,
                    signature,
                });
            }
        }
        methods
    }

    fn imported_inheritance_signature(
        &mut self,
        name: &str,
        callable: &hir::CallableDeclarationRecordV1,
        arguments: &[TypeId],
    ) -> Option<InterfaceSignature> {
        let bindings = arguments
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                (
                    scoop_identity::SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    *ty,
                )
            })
            .collect::<crate::imported_core::ImportedTypeBindings>();
        let effects = callable.effects();
        Some(InterfaceSignature {
            name: name.to_owned(),
            parameters: callable
                .parameters()
                .parameters()
                .iter()
                .map(|parameter| {
                    self.imported_signature_type_with_bindings(parameter.value_type(), &bindings)
                        .ok()
                })
                .collect::<Option<Vec<_>>>()?,
            result: self
                .imported_signature_type_with_bindings(callable.result(), &bindings)
                .ok()?,
            is_suspend: effects.execution() == scoop_identity::Effect::Suspend,
            safety: effects.safety(),
            gc_effect: effects.gc_effect(),
            operator: effects.operator_role(),
            infix: effects.infix(),
        })
    }

    pub(super) fn check_imported_class_override(
        &mut self,
        id: FunctionId,
        declaration: &ast::FunctionDecl,
        signature: &FnSig,
        inherited: &ImportedInheritedMethod,
    ) {
        let target = format!(
            "{}.{}",
            self.type_name(inherited.owner),
            inherited.signature.name
        );
        if let Some(required) =
            self.imported_callable_slot_domain(inherited.declaration.interface())
        {
            let provided = &mut self.functions[id].access;
            if provided.declared == hir::DeclaredVisibility::Protected
                && inherited.declaration.interface().declared_visibility()
                    == hir::DeclaredVisibilityV1::Protected
            {
                provided.slot = Some(required.clone());
            }
            let covers = provided
                .slot
                .clone()
                .is_some_and(|provided| self.access_domain_is_subset(&required.0, &provided.0));
            if !covers {
                self.error(
                    declaration.name.span,
                    format!(
                        "visibility of `{}` does not cover inherited slot `{target}`",
                        declaration.name.text
                    ),
                );
            }
        }
        for (index, parameter) in signature.params.iter().enumerate() {
            let inherited_vararg = inherited
                .declaration
                .source_interface()
                .is_some_and(|source| {
                    source.parameters().parameters()[index]
                        .calling()
                        .is_vararg()
                });
            if matches!(parameter.calling, crate::FnParamCalling::Vararg { .. }) != inherited_vararg
            {
                self.error(
                    declaration.params[index].span,
                    format!(
                        "parameter `{}` of `{}` must have the same `vararg` shape as `{target}`",
                        declaration.params[index].name.text, declaration.name.text
                    ),
                );
                break;
            }
        }
        self.override_default_sources.entry(id).or_default().push(
            crate::defaults::DefaultOverrideSource::Imported {
                declaration: inherited.declaration.interface().declaration(),
                owner: inherited.owner,
            },
        );
    }
}
