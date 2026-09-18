use scoop_ast as ast;
use scoop_hir as hir;

use super::*;
use crate::call_resolution::specificity::DeclarationForwardingView;

pub(crate) struct ImportedDependencyExtensionPropertyProbe {
    state: Box<Lowerer>,
    binding: hir::DirectImportedTargetBinding,
    receiver: hir::Expr,
    receiver_type: hir::TypeId,
    value_type: hir::TypeId,
    has_setter: bool,
    declaration_file: usize,
    declaration_span: ast::Span,
}

pub(crate) struct ImportedDependencyExtensionPropertySelection {
    pub(crate) binding: hir::DirectImportedTargetBinding,
    pub(crate) receiver: hir::Expr,
    pub(crate) value_type: hir::TypeId,
    pub(crate) has_setter: bool,
}

impl ImportedDependencyExtensionPropertyProbe {
    pub(crate) fn forwarding(&self) -> DeclarationForwardingView<'_> {
        DeclarationForwardingView::nominal_parameters(
            &[],
            std::slice::from_ref(&self.receiver_type),
        )
    }

    pub(crate) const fn parameterized(&self) -> bool {
        false
    }

    pub(crate) fn signature(&self, state: &Lowerer, name: &str) -> String {
        format!(
            "{}.{}: {}",
            state.type_name(self.receiver_type),
            name,
            state.type_name(self.value_type),
        )
    }

    pub(crate) const fn declaration_location(&self) -> (usize, ast::Span) {
        (self.declaration_file, self.declaration_span)
    }
}

impl Lowerer {
    pub(crate) fn probe_imported_dependency_extension_property(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        receiver: hir::Expr,
        name: &ast::Ident,
        require_getter_capability: bool,
    ) -> Result<ImportedDependencyExtensionPropertyProbe, Box<Lowerer>> {
        let mut state = self.clone();
        let property = match state.imported_dependency_property_candidate(binding, name.span) {
            Some(property) => property,
            None => return Err(Box::new(state)),
        };
        if property.interface().owner() != hir::PublicDeclarationOwnerV1::Extension {
            state.error(
                name.span,
                format!(
                    "dependency property `{}` is not an extension property",
                    name.text
                ),
            );
            return Err(Box::new(state));
        }
        if !property.interface().type_parameters().is_empty() {
            state.error(
                name.span,
                ImportedCapabilityRequirement::Generic.diagnostic("dependency extension property"),
            );
            return Err(Box::new(state));
        }
        let Some(receiver_signature) = property.interface().receiver() else {
            state.error(
                name.span,
                format!(
                    "invalid dependency extension property `{}`: receiver type is missing",
                    name.text
                ),
            );
            return Err(Box::new(state));
        };
        let Some(receiver_type) = state.imported_property_signature_type(
            receiver_signature,
            "dependency extension property receiver",
            name.span,
        ) else {
            return Err(Box::new(state));
        };
        if !state.is_subtype(receiver.ty, receiver_type) {
            state.error(
                name.span,
                format!(
                    "extension property `{}` expects receiver {}, found {}",
                    name.text,
                    state.type_name(receiver_type),
                    state.type_name(receiver.ty),
                ),
            );
            return Err(Box::new(state));
        }
        let Some(value_type) = state.imported_property_value_type(&property, name.span) else {
            return Err(Box::new(state));
        };
        if require_getter_capability
            && state
                .imported_dependency_property_accessor(
                    &property,
                    hir::ImportedDependencyPropertyAccessorKind::Getter,
                    "dependency extension property getter",
                    name.span,
                )
                .is_none()
        {
            return Err(Box::new(state));
        }
        let receiver = state.adapt_to(receiver, receiver_type);
        Ok(ImportedDependencyExtensionPropertyProbe {
            declaration_file: state.current_file,
            declaration_span: name.span,
            state: Box::new(state),
            binding: binding.clone(),
            receiver,
            receiver_type,
            value_type,
            has_setter: property.interface().capability().setter().is_some(),
        })
    }

    pub(crate) fn commit_imported_dependency_extension_property(
        &mut self,
        probe: ImportedDependencyExtensionPropertyProbe,
    ) -> ImportedDependencyExtensionPropertySelection {
        let ImportedDependencyExtensionPropertyProbe {
            state,
            binding,
            receiver,
            value_type,
            has_setter,
            ..
        } = probe;
        *self = *state;
        ImportedDependencyExtensionPropertySelection {
            binding,
            receiver,
            value_type,
            has_setter,
        }
    }
}
