//! Runtime cast requirements use the actual exception declaration.

use scoop_ast::{Diagnostic, Span};
use scoop_identity::SignatureTypeKey;

use super::*;
use crate::imported_core::ImportedSignatureTypeError;
use crate::{CoreLoweringAuthority, Lowerer};

impl Lowerer {
    pub(crate) fn prepare_cast_exception_type(&mut self) -> Result<(), ImportedSignatureTypeError> {
        let CoreLoweringAuthority::Imported(imported) = &self.core else {
            return Ok(());
        };
        let declaration = imported
            .protocols
            .exceptions()
            .class_cast_exception()
            .persistent();
        if self.types.iter().any(|(_, ty)| {
            matches!(ty, export::Type::ImportedClass(class) if class.declaration.identity.id() == declaration)
        }) {
            return Ok(());
        }

        // An inactive source default may name an exception with generic storage.
        // Commit only complete types; concrete uses are checked before output.
        let mut candidate = self.clone();
        match candidate.imported_signature_type(&SignatureTypeKey::Nominal(declaration)) {
            Ok(_) => {
                *self = candidate;
                Ok(())
            }
            Err(ImportedSignatureTypeError::Generic) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Concretizer<'_> {
    pub(super) fn lower_cast_exception_type(&mut self) {
        let CoreConcretizationAuthority::Imported(protocols) = self.core else {
            return;
        };
        let declaration = protocols.exceptions().class_cast_exception().persistent();
        if let Some((ty, _)) = self.source.types.iter().find(|(_, ty)| {
            matches!(ty, export::Type::ImportedClass(class) if class.declaration.identity.id() == declaration)
        }) {
            self.lower_type(ty, &[]);
        }
    }
}

pub(super) fn check_runtime_layout(module: &concrete::Module) -> Result<(), Vec<Diagnostic>> {
    let concrete::ConcreteCoreProtocols::Imported(protocols) = &module.core_protocols else {
        return Ok(());
    };
    let declaration = protocols.exceptions().class_cast_exception().persistent();
    if module
        .classes
        .iter()
        .any(|(_, class)| class.origin.concrete_type_id() == Some(declaration))
    {
        return Ok(());
    }
    module.visit_executable_expressions(|occurrence| {
        if matches!(occurrence.expression.kind, concrete::ExprKind::Cast { optional: false, .. }) {
            return Err(Diagnostic::at(occurrence.expression.span,
                "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED: runtime cast failure constructor requires a materialized dependency layout; its owner has source-only representation"));
        }
        Ok(())
    }).map_err(|error| vec![match error {
        concrete::ExecutableExpressionVisitError::Visitor(diagnostic) => diagnostic,
        concrete::ExecutableExpressionVisitError::Structure(error) => Diagnostic::at(
            Span::new(0, 0), format!("cannot inspect runtime constructor requirements: {error}")),
    }])
}
