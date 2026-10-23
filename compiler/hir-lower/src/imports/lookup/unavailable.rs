//! Incomplete declaration bindings are lookup barriers, never language entities.

use super::*;

pub(crate) enum UnavailableBindingKind {
    Declaration(ast::DeclarationHeaderKind),
    Import,
}

impl Lowerer {
    pub(crate) fn unavailable_binding_kind(
        &self,
        binding: CurrentUnitBindingId,
    ) -> Option<UnavailableBindingKind> {
        let target = self.imports.binding(binding).target;
        if self.editing_declarations.is_some()
            && let Some(target) = target.type_target()
        {
            let observed = self.editing_lookup_type_target(&TypeLookupTarget::Current(target));
            if !self.editing_type_available(observed) {
                return Some(UnavailableBindingKind::Declaration(match target {
                    crate::namespace::TopLevelTypeTarget::Nominal(
                        crate::NominalTarget::Object(_),
                    ) => ast::DeclarationHeaderKind::Singleton,
                    _ => ast::DeclarationHeaderKind::Type,
                }));
            }
        }
        if self.editing_declarations.is_some()
            && matches!(
                target,
                CurrentUnitTarget::Property(_)
                    | CurrentUnitTarget::SourceProperty(_)
                    | CurrentUnitTarget::EnumVariant(_)
                    | CurrentUnitTarget::SourceVariant(_)
            )
        {
            let declaration = self.imports.binding(binding);
            if !self.editing_declaration_scope_available(&hir::SemanticSourceLocation {
                source: declaration.source.clone(),
                span: declaration.span,
            }) {
                return Some(UnavailableBindingKind::Declaration(
                    ast::DeclarationHeaderKind::Property {
                        is_extension: false,
                    },
                ));
            }
        }
        match self.imports.binding(binding).target {
            CurrentUnitTarget::Unavailable(kind) => Some(UnavailableBindingKind::Declaration(kind)),
            CurrentUnitTarget::UnavailableImport => Some(UnavailableBindingKind::Import),
            CurrentUnitTarget::Function(function) => self
                .declaration_surface
                .unavailable_signature(function)
                .map(|signature| UnavailableBindingKind::Declaration(signature.kind)),
            _ => None,
        }
    }

    pub(crate) fn unavailable_import_bindings(
        &self,
        bindings: &[CurrentUnitBindingId],
        namespace: Option<scoop_identity::BindingNamespace>,
    ) -> Vec<CurrentUnitBindingId> {
        self.unavailable_bindings_matching(bindings, |kind| {
            namespace.is_none_or(|namespace| kind.namespaces().contains(namespace))
        })
    }

    pub(crate) fn unavailable_bindings_matching(
        &self,
        bindings: &[CurrentUnitBindingId],
        matches: impl Fn(ast::DeclarationHeaderKind) -> bool,
    ) -> Vec<CurrentUnitBindingId> {
        bindings
            .iter()
            .copied()
            .filter(|&binding| {
                let declaration = self.imports.binding(binding);
                let Some(kind) = self.unavailable_binding_kind(binding) else {
                    return false;
                };
                (match kind {
                    UnavailableBindingKind::Declaration(kind) => matches(kind),
                    UnavailableBindingKind::Import => true,
                }) && self.declaration_source_in_scope(declaration.file)
                    && self.access_domain_allows(&declaration.access.0)
            })
            .collect()
    }

    pub(crate) fn unavailable_implicit_values(
        &self,
        bindings: &[CurrentUnitBindingId],
    ) -> Vec<CurrentUnitBindingId> {
        let has_receiver = !self.implicit_receivers().is_empty();
        self.unavailable_bindings_matching(bindings, |kind| {
            kind.is_value()
                || (matches!(
                    kind,
                    ast::DeclarationHeaderKind::Property { is_extension: true }
                ) && has_receiver)
        })
    }

    pub(crate) fn unavailable_named_call_candidates(
        &self,
        bindings: &[CurrentUnitBindingId],
    ) -> Vec<CurrentUnitBindingId> {
        let has_receiver = !self.implicit_receivers().is_empty();
        self.unavailable_bindings_matching(bindings, |kind| {
            has_receiver
                || !matches!(
                    kind,
                    ast::DeclarationHeaderKind::Function {
                        is_extension: true,
                        ..
                    } | ast::DeclarationHeaderKind::Property { is_extension: true }
                )
        })
    }

    pub(crate) fn unavailable_extension_candidates(
        &self,
        bindings: &[CurrentUnitBindingId],
        required: crate::expr::RequiredCallableModifiers,
    ) -> Vec<CurrentUnitBindingId> {
        use crate::callable_modifiers::{CallableOperatorKind, callable_operator_kind};
        self.unavailable_bindings_matching(bindings, |kind| {
            let ast::DeclarationHeaderKind::Function {
                is_extension: true,
                is_operator,
                is_infix,
            } = kind
            else {
                return false;
            };
            (!required.infix || is_infix)
                && ((required.operator.is_none() && required.property_delegate_operator.is_none())
                    || is_operator)
        })
        .into_iter()
        .filter(|&binding| {
            if matches!(
                self.imports.binding(binding).target,
                CurrentUnitTarget::UnavailableImport
            ) {
                return true;
            }
            let role = callable_operator_kind(&self.imports.binding(binding).name).ok();
            required
                .operator
                .is_none_or(|operator| role == Some(CallableOperatorKind::Ordinary(operator)))
                && required.property_delegate_operator.is_none_or(|operator| {
                    role == Some(CallableOperatorKind::PropertyDelegate(operator))
                })
        })
        .collect()
    }

    pub(crate) fn report_unavailable_lookup(
        &mut self,
        name: &ast::Ident,
        bindings: &[CurrentUnitBindingId],
    ) -> bool {
        if bindings.is_empty() {
            return false;
        }
        let bindings = self.imports.canonicalize(bindings.iter().copied());
        let has_import_failure = bindings.iter().any(|&binding| {
            matches!(
                self.imports.binding(binding).target,
                CurrentUnitTarget::UnavailableImport
            )
        });
        let mut diagnostic = ast::Diagnostic::at_file(
            self.current_file,
            name.span,
            if has_import_failure {
                format!("import binding for `{}` is unavailable", name.text)
            } else {
                format!("declaration `{}` is incomplete", name.text)
            },
        );
        diagnostic.notes.extend(bindings.iter().map(|&binding| {
            let declaration = self.imports.binding(binding);
            ast::DiagnosticNote::at(
                declaration.file,
                declaration.span,
                if matches!(declaration.target, CurrentUnitTarget::UnavailableImport) {
                    "this import could not be resolved"
                } else {
                    "this declaration has no complete semantic input"
                },
            )
        }));
        self.diagnostics.push(diagnostic);
        true
    }
}
