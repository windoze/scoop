//! Incomplete declaration bindings are lookup barriers, never language entities.

use super::*;

impl Lowerer {
    pub(crate) fn unavailable_binding_kind(
        &self,
        binding: CurrentUnitBindingId,
    ) -> Option<ast::DeclarationHeaderKind> {
        match self.imports.binding(binding).target {
            CurrentUnitTarget::Unavailable(kind) => Some(kind),
            CurrentUnitTarget::Function(function) => self
                .declaration_surface
                .unavailable_signature(function)
                .map(|signature| signature.kind),
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
                matches(kind)
                    && self.declaration_source_in_scope(declaration.file)
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
        let mut diagnostic = ast::Diagnostic::at_file(
            self.current_file,
            name.span,
            format!("declaration `{}` is incomplete", name.text),
        );
        diagnostic.notes.extend(bindings.iter().map(|&binding| {
            let declaration = self.imports.binding(binding);
            ast::DiagnosticNote::at(
                declaration.file,
                declaration.span,
                "this declaration has no complete semantic input",
            )
        }));
        self.diagnostics.push(diagnostic);
        true
    }
}
