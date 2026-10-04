//! Runtime failures use their actual exception declarations.

use scoop_ast::{Diagnostic, Span};
use scoop_identity::{PersistentTypeId, SignatureTypeKey};

use super::*;
use crate::imported_core::ImportedSignatureTypeError;
use crate::{CoreLoweringAuthority, Lowerer};

impl Lowerer {
    pub(crate) fn prepare_unwrap_exception_type(
        &mut self,
    ) -> Result<(), ImportedSignatureTypeError> {
        let CoreLoweringAuthority::Imported(imported) = &self.core else {
            return Ok(());
        };
        self.prepare_runtime_exception_type(imported.exceptions().unwrap_exception().persistent())
    }

    pub(crate) fn prepare_cast_exception_type(&mut self) -> Result<(), ImportedSignatureTypeError> {
        let CoreLoweringAuthority::Imported(imported) = &self.core else {
            return Ok(());
        };
        let declaration = imported.exceptions().class_cast_exception().persistent();
        self.prepare_runtime_exception_type(declaration)
    }

    pub(crate) fn prepare_arithmetic_exception_type(
        &mut self,
    ) -> Result<(), ImportedSignatureTypeError> {
        let CoreLoweringAuthority::Imported(imported) = &self.core else {
            return Ok(());
        };
        let declaration = imported.exceptions().arithmetic_exception().persistent();
        self.prepare_runtime_exception_type(declaration)
    }

    pub(crate) fn prepare_array_bounds_exception_type(
        &mut self,
    ) -> Result<(), ImportedSignatureTypeError> {
        let CoreLoweringAuthority::Imported(imported) = &self.core else {
            return Ok(());
        };
        let declaration = imported
            .exceptions()
            .index_out_of_bounds_exception()
            .persistent();
        self.prepare_runtime_exception_type(declaration)
    }

    pub(crate) fn prepare_array_size_exception_type(
        &mut self,
    ) -> Result<(), ImportedSignatureTypeError> {
        let CoreLoweringAuthority::Imported(imported) = &self.core else {
            return Ok(());
        };
        let declaration = imported
            .exceptions()
            .illegal_argument_exception()
            .persistent();
        self.prepare_runtime_exception_type(declaration)
    }

    pub(crate) fn prepare_runtime_exception_type(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<(), ImportedSignatureTypeError> {
        if self.types.iter().any(|(_, ty)| {
            matches!(ty, export::Type::Class(class) if self.class_applications[*class].template == export::SourceNominalId::Concrete(declaration))
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
    pub(super) fn lower_unwrap_exception_type(&mut self) {
        let export::CoreProtocols::Imported(protocols) = self.core else {
            return;
        };
        self.lower_runtime_exception_type(protocols.exceptions().unwrap_exception().persistent());
    }

    pub(super) fn lower_cast_exception_type(&mut self) {
        let export::CoreProtocols::Imported(protocols) = self.core else {
            return;
        };
        let declaration = protocols.exceptions().class_cast_exception().persistent();
        self.lower_runtime_exception_type(declaration);
    }

    pub(super) fn lower_arithmetic_exception_type(&mut self) {
        let export::CoreProtocols::Imported(protocols) = self.core else {
            return;
        };
        let declaration = protocols.exceptions().arithmetic_exception().persistent();
        self.lower_runtime_exception_type(declaration);
    }

    pub(super) fn lower_array_bounds_exception_type(&mut self) {
        let export::CoreProtocols::Imported(protocols) = self.core else {
            return;
        };
        let declaration = protocols
            .exceptions()
            .index_out_of_bounds_exception()
            .persistent();
        self.lower_runtime_exception_type(declaration);
    }

    pub(super) fn lower_array_size_exception_type(&mut self) {
        let export::CoreProtocols::Imported(protocols) = self.core else {
            return;
        };
        let declaration = protocols
            .exceptions()
            .illegal_argument_exception()
            .persistent();
        self.lower_runtime_exception_type(declaration);
    }

    pub(super) fn lower_runtime_exception_type(&mut self, declaration: PersistentTypeId) {
        if let Some((ty, _)) = self.source.types.iter().find(|(_, ty)| {
            matches!(ty, export::Type::Class(class) if self.source.class_applications[*class].template == export::SourceNominalId::Concrete(declaration))
        }) {
            self.lower_type(ty, &[]);
        }
    }
}

pub(super) fn check_runtime_layout(module: &concrete::Module) -> Result<(), Vec<Diagnostic>> {
    let concrete::ConcreteCoreProtocols::Imported(protocols) = &module.core_protocols else {
        return Ok(());
    };
    let has_layout = |declaration| {
        module
            .classes
            .iter()
            .any(|(_, class)| class.origin.concrete_type_id() == Some(declaration))
    };
    let cast_layout = has_layout(protocols.exceptions().class_cast_exception().persistent());
    let arithmetic_layout = has_layout(protocols.exceptions().arithmetic_exception().persistent());
    let unwrap_layout = has_layout(protocols.exceptions().unwrap_exception().persistent());
    let bounds_layout = has_layout(
        protocols
            .exceptions()
            .index_out_of_bounds_exception()
            .persistent(),
    );
    let size_layout = has_layout(
        protocols
            .exceptions()
            .illegal_argument_exception()
            .persistent(),
    );
    if cast_layout && arithmetic_layout && unwrap_layout && bounds_layout && size_layout {
        return Ok(());
    }
    module.visit_executable_expressions(|occurrence| {
        let operation = match occurrence.expression.kind {
            concrete::ExprKind::ArrayGenerate { .. } if !size_layout => "array size exception constructor",
            concrete::ExprKind::Unwrap { trap_on_none: true, .. } if !unwrap_layout => "Option unwrap exception constructor",
            concrete::ExprKind::Cast { optional: false, .. } if !cast_layout => "runtime cast failure constructor",
            concrete::ExprKind::Index { .. } | concrete::ExprKind::ArraySet { .. }
                if !bounds_layout => "array bounds exception constructor",
            concrete::ExprKind::IntegerOperation {
                operation: concrete::IntegerOperation::Managed { .. }, ..
            } if !arithmetic_layout => "integer division exception constructor",
            concrete::ExprKind::CallableReference(id) if !arithmetic_layout && matches!(
                module.callable_references[id].target,
                concrete::CallableReferenceTarget::BoundIntrinsic { intrinsic, .. }
                    if intrinsic.requires_arithmetic_exception()
            ) => "integer reference exception constructor",
            _ => return Ok(()),
        };
        let evaluation = occurrence.expression.origin.evaluation;
        Err(Diagnostic::at_file(evaluation.file as usize, evaluation.span, format!(
            "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED: {operation} requires a materialized dependency layout; its owner has source-only representation")))
    }).map_err(|error| vec![match error {
        concrete::ExecutableExpressionVisitError::Visitor(diagnostic) => diagnostic,
        concrete::ExecutableExpressionVisitError::Structure(error) => Diagnostic::at(
            Span::new(0, 0), format!("cannot inspect runtime constructor requirements: {error}")),
    }])
}
