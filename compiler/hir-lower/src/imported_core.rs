//! Imported-core prelude candidates and committed HIR uses.

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{
    BindingNamespace, NominalDeclarationOwner, PersistentTypeId, SignatureTypeKey,
};

use crate::{CoreLoweringAuthority, Lowerer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImportedSignatureTypeError {
    Generic,
    Structural,
}

impl Lowerer {
    /// Retains the shared public route for a built-in spelling in an alias.
    pub(crate) fn retain_builtin_alias_target_binding(
        &mut self,
        name: &str,
        declaration: PersistentTypeId,
        span: Span,
    ) -> bool {
        if self.type_alias_resolution_stack.is_empty()
            || matches!(self.core, CoreLoweringAuthority::Defined)
        {
            return true;
        }
        let binding = self
            .imports
            .prelude_bindings(BindingNamespace::Type, name)
            .iter()
            .find(|binding| matches!(binding.target(), hir::ImportedTarget::Type(id) if id.persistent() == declaration))
            .cloned();
        let Some(binding) = binding else {
            self.error(
                span,
                format!("core prelude does not expose type binding `{name}`"),
            );
            return false;
        };
        self.retain_imported_alias_target_bindings(
            &binding,
            hir::ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(declaration)),
        );
        true
    }

    pub(crate) fn imported_core_builtin_declaration(
        &self,
        ty: hir::TypeId,
    ) -> Option<PersistentTypeId> {
        let CoreLoweringAuthority::Imported(authority) = &self.core else {
            return None;
        };
        let fundamental = authority.protocols.fundamental_types();
        Some(match self.types[ty] {
            hir::Type::Unit => fundamental.unit().persistent(),
            hir::Type::Integer(kind) => fundamental.integer(kind).persistent(),
            hir::Type::Boolean => fundamental.boolean().persistent(),
            hir::Type::String => fundamental.string().persistent(),
            hir::Type::Any => scoop_identity::CoreBuiltinNominal::Any
                .identity_record()
                .id(),
            hir::Type::Struct(_)
            | hir::Type::Class(_)
            | hir::Type::Interface(_)
            | hir::Type::Tuple(_)
            | hir::Type::Function(_)
            | hir::Type::Ptr(_)
            | hir::Type::FunPtr(_)
            | hir::Type::Enum(_)
            | hir::Type::Param(_) => return None,
        })
    }

    pub(crate) fn imported_signature_type(
        &mut self,
        signature: &SignatureTypeKey,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        match signature {
            SignatureTypeKey::Nominal(identity) => {
                if *identity
                    == scoop_identity::CoreBuiltinNominal::Unit
                        .identity_record()
                        .id()
                {
                    return Ok(self.unit);
                }
                if *identity
                    == scoop_identity::CoreBuiltinNominal::Any
                        .identity_record()
                        .id()
                {
                    return Ok(self.any);
                }
                let CoreLoweringAuthority::Imported(authority) = &self.core else {
                    return Err(ImportedSignatureTypeError::Structural);
                };
                let fundamental = authority.protocols.fundamental_types();
                if let Some(kind) = hir::IntegerKind::ALL
                    .into_iter()
                    .find(|kind| *identity == fundamental.integer(*kind).persistent())
                {
                    Ok(self.integer_type(kind))
                } else if *identity == fundamental.boolean().persistent() {
                    Ok(self.boolean)
                } else if *identity == fundamental.string().persistent() {
                    Ok(self.string)
                } else {
                    Err(ImportedSignatureTypeError::Structural)
                }
            }
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                Err(ImportedSignatureTypeError::Generic)
            }
            SignatureTypeKey::Tuple(elements) => {
                let elements = elements
                    .as_slice()
                    .iter()
                    .map(|element| self.imported_signature_type(element))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.intern_type(hir::Type::Tuple(elements)))
            }
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => {
                let parameters = parameters
                    .iter()
                    .map(|parameter| self.imported_signature_type(parameter))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.imported_signature_type(result)?;
                Ok(self.intern_function_type(
                    *effect == scoop_identity::Effect::Suspend,
                    parameters,
                    result,
                ))
            }
            SignatureTypeKey::RawPointer(pointee) => {
                let pointee = self.imported_signature_type(pointee)?;
                Ok(self.intern_type(hir::Type::Ptr(pointee)))
            }
            SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                let parameters = parameters
                    .iter()
                    .map(|parameter| self.imported_signature_type(parameter))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.imported_signature_type(result)?;
                let signature = self.intern_function_type(false, parameters, result);
                let hir::Type::Function(function) = self.types[signature] else {
                    unreachable!("interned function signatures have function types")
                };
                Ok(self.intern_type(hir::Type::FunPtr(function)))
            }
        }
    }
}
