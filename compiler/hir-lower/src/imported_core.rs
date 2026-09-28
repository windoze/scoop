//! Prelude references and shared dependency signature types.

mod interfaces;
mod intrinsics;
mod members;
mod nominals;
mod pointers;

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

pub(crate) type ImportedTypeBindings = std::collections::BTreeMap<SignatureTypeKey, hir::TypeId>;

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

    pub(crate) fn imported_nominal_declaration(&self, ty: hir::TypeId) -> Option<PersistentTypeId> {
        if let hir::Type::ImportedStruct(ty) = &self.types[ty] {
            return ty.declaration.identity.concrete_id();
        }
        if let hir::Type::ImportedEnum(ty) = &self.types[ty] {
            return ty.declaration.identity.concrete_id();
        }
        if let hir::Type::ImportedClass(ty) = &self.types[ty] {
            return ty.declaration.identity.concrete_id();
        }
        if let hir::Type::ImportedInterface(ty) = &self.types[ty] {
            return ty.declaration.identity.concrete_id();
        }
        let CoreLoweringAuthority::Imported(authority) = &self.core else {
            return None;
        };
        let fundamental = authority.fundamental_types();
        Some(match self.types[ty] {
            hir::Type::Unit => fundamental.unit().persistent(),
            hir::Type::Integer(kind) => fundamental.integer(kind).persistent(),
            hir::Type::Boolean => fundamental.boolean().persistent(),
            hir::Type::String => fundamental.string().persistent(),
            hir::Type::Any => scoop_identity::CoreBuiltinNominal::Any
                .identity_record()
                .id(),
            hir::Type::ImportedStruct(_)
            | hir::Type::ImportedEnum(_)
            | hir::Type::ImportedClass(_)
            | hir::Type::ImportedInterface(_)
            | hir::Type::Struct(_)
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
        self.imported_signature_type_with_bindings(signature, &ImportedTypeBindings::new())
    }

    pub(crate) fn imported_signature_type_with_bindings(
        &mut self,
        signature: &SignatureTypeKey,
        bindings: &ImportedTypeBindings,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        if let Some(ty) = bindings.get(signature) {
            return Ok(*ty);
        }
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
                if let CoreLoweringAuthority::Imported(authority) = &self.core {
                    let fundamental = authority.fundamental_types();
                    if let Some(kind) = hir::IntegerKind::ALL
                        .into_iter()
                        .find(|kind| *identity == fundamental.integer(*kind).persistent())
                    {
                        return Ok(self.integer_type(kind));
                    }
                    if *identity == fundamental.boolean().persistent() {
                        return Ok(self.boolean);
                    }
                    if *identity == fundamental.string().persistent() {
                        return Ok(self.string);
                    }
                }
                self.imported_nominal_type(*identity)
            }
            SignatureTypeKey::NominalApplication { origin, arguments } => {
                let arguments = arguments
                    .as_slice()
                    .iter()
                    .map(|argument| self.imported_signature_type_with_bindings(argument, bindings))
                    .collect::<Result<Vec<_>, _>>()?;
                self.imported_nominal_application(
                    hir::SourceNominalId::GenericTemplate(*origin),
                    arguments,
                )
            }
            SignatureTypeKey::Binder { .. } => Err(ImportedSignatureTypeError::Generic),
            SignatureTypeKey::Tuple(elements) => {
                let elements = elements
                    .as_slice()
                    .iter()
                    .map(|element| self.imported_signature_type_with_bindings(element, bindings))
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
                    .map(|parameter| {
                        self.imported_signature_type_with_bindings(parameter, bindings)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.imported_signature_type_with_bindings(result, bindings)?;
                Ok(self.intern_function_type(
                    *effect == scoop_identity::Effect::Suspend,
                    parameters,
                    result,
                ))
            }
            SignatureTypeKey::RawPointer(pointee) => {
                let pointee = self.imported_signature_type_with_bindings(pointee, bindings)?;
                Ok(self.intern_type(hir::Type::Ptr(pointee)))
            }
            SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                let parameters = parameters
                    .iter()
                    .map(|parameter| {
                        self.imported_signature_type_with_bindings(parameter, bindings)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.imported_signature_type_with_bindings(result, bindings)?;
                let signature = self.intern_function_type(false, parameters, result);
                let hir::Type::Function(function) = self.types[signature] else {
                    unreachable!("interned function signatures have function types")
                };
                Ok(self.intern_type(hir::Type::FunPtr(function)))
            }
        }
    }
}
