//! Dependency property access for the M23-5 executable subset.
//!
//! Public property metadata selects an accessor; only the accessor becomes a
//! machine-level dependency call. Provider storage and initialization details
//! never enter the consumer HIR.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;
use crate::imported_capabilities::{ImportedCapabilityRequirement, callable_requirement};
use crate::properties::PropertyCallReceiver;

mod extension;
mod read;
mod write;

pub(crate) use extension::ImportedDependencyExtensionPropertyProbe;

pub(crate) struct ImportedDependencyPropertyRead {
    pub(crate) expression: hir::Expr,
    pub(crate) has_setter: bool,
}

struct PreparedImportedPropertySetter {
    candidate: hir::ImportedDependencyCallableCandidate,
    receiver: Option<PropertyCallReceiver>,
    value_type: hir::TypeId,
}

impl Lowerer {
    fn imported_dependency_property_candidate(
        &mut self,
        binding: &hir::DirectImportedTargetBinding,
        span: ast::Span,
    ) -> Option<hir::ImportedDependencyPropertyCandidate> {
        match self
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .property_candidate(binding)
        {
            Ok(candidate) => Some(candidate),
            Err(error) => {
                self.error(
                    span,
                    format!("invalid imported dependency property: {error}"),
                );
                None
            }
        }
    }

    fn imported_dependency_property_accessor(
        &mut self,
        property: &hir::ImportedDependencyPropertyCandidate,
        kind: hir::ImportedDependencyPropertyAccessorKind,
        subject: &str,
        span: ast::Span,
    ) -> Option<hir::ImportedDependencyCallableCandidate> {
        let candidate = match self
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .property_accessor_candidate(property, kind)
        {
            Ok(candidate) => candidate,
            Err(error) => {
                self.error(
                    span,
                    format!("invalid imported dependency property: {error}"),
                );
                return None;
            }
        };
        if candidate.capability().is_none() {
            let requirement = if matches!(
                property.interface().owner(),
                hir::PublicDeclarationOwnerV1::Nominal(_)
            ) {
                ImportedCapabilityRequirement::Dispatch
            } else if !property.interface().type_parameters().is_empty() {
                ImportedCapabilityRequirement::Generic
            } else {
                callable_requirement(&candidate, false)
            };
            self.error(span, requirement.diagnostic(subject));
            return None;
        }
        Some(candidate)
    }

    fn validate_imported_property_receiver(
        &mut self,
        property: &hir::ImportedDependencyPropertyCandidate,
        receiver: Option<PropertyCallReceiver>,
        span: ast::Span,
    ) -> Option<Option<PropertyCallReceiver>> {
        match (property.interface().owner(), receiver) {
            (hir::PublicDeclarationOwnerV1::TopLevel, None) => Some(None),
            (hir::PublicDeclarationOwnerV1::Extension, Some(receiver)) => Some(Some(receiver)),
            (hir::PublicDeclarationOwnerV1::Nominal(_), _) => {
                self.error(
                    span,
                    ImportedCapabilityRequirement::Dispatch
                        .diagnostic("dependency property access"),
                );
                None
            }
            (hir::PublicDeclarationOwnerV1::TopLevel, Some(_))
            | (hir::PublicDeclarationOwnerV1::Extension, None) => {
                self.error(
                    span,
                    "invalid imported dependency property receiver".to_string(),
                );
                None
            }
        }
    }

    fn imported_property_value_type(
        &mut self,
        property: &hir::ImportedDependencyPropertyCandidate,
        span: ast::Span,
    ) -> Option<hir::TypeId> {
        self.imported_property_signature_type(
            property.interface().value_type(),
            "dependency property value type",
            span,
        )
    }

    fn imported_property_signature_type(
        &mut self,
        signature: &scoop_identity::SignatureTypeKey,
        subject: &str,
        span: ast::Span,
    ) -> Option<hir::TypeId> {
        match self.imported_signature_type(signature) {
            Ok(ty) => Some(ty),
            Err(crate::imported_core::ImportedSignatureTypeError::Generic) => {
                self.error(
                    span,
                    ImportedCapabilityRequirement::Generic.diagnostic(subject),
                );
                None
            }
            Err(crate::imported_core::ImportedSignatureTypeError::Structural) => {
                self.error(
                    span,
                    ImportedCapabilityRequirement::Layout.diagnostic(subject),
                );
                None
            }
        }
    }

    fn emit_imported_property_accessor(
        &mut self,
        candidate: hir::ImportedDependencyCallableCandidate,
        args: Vec<hir::Expr>,
        receiver: hir::SourceCallReceiver<hir::TypeId>,
        result_type: hir::TypeId,
        span: ast::Span,
        unsafe_operation: &str,
    ) -> Option<hir::Expr> {
        if candidate.interface().effects().safety() == hir::CallableSafetyV1::Unsafe {
            self.require_unsafe_operation(span, unsafe_operation);
        }
        let (callee, binding) = match self.select_imported_dependency_callable_use(candidate) {
            Ok(selected) => selected,
            Err(error) => {
                self.error(
                    span,
                    format!("failed to select imported dependency accessor: {error}"),
                );
                return None;
            }
        };
        Some(hir::Expr {
            kind: hir::ExprKind::ImportedDependencyCall {
                callee,
                binding: Some(binding),
                args,
                receiver,
            },
            ty: result_type,
            span,
            origin: self.expression_origin(span),
        })
    }
}
