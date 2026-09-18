//! Imported-core prelude candidates and committed HIR uses.

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{BindingNamespace, SignatureTypeKey};

use crate::{CoreLoweringAuthority, Lowerer};

#[derive(Clone)]
pub(crate) struct ImportedCoreCallableCandidate {
    pub(crate) reference: hir::ImportedCorePreludeRef,
    pub(crate) target: hir::CoreCallableTargetV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImportedSignatureTypeError {
    Generic,
    Structural,
}

impl Lowerer {
    pub(crate) fn imported_core_callable_candidates(
        &self,
        name: &str,
    ) -> Vec<ImportedCoreCallableCandidate> {
        let CoreLoweringAuthority::Imported(authority) = &self.core else {
            return Vec::new();
        };
        authority
            .candidates
            .iter()
            .filter(|candidate| {
                candidate.namespace == BindingNamespace::Value && candidate.name == name
            })
            .map(|candidate| ImportedCoreCallableCandidate {
                reference: candidate.reference,
                target: candidate.target.clone(),
            })
            .collect()
    }

    pub(crate) fn imported_core_callable_candidate(
        &self,
        reference: hir::ImportedCorePreludeRef,
    ) -> ImportedCoreCallableCandidate {
        let CoreLoweringAuthority::Imported(authority) = &self.core else {
            panic!("defined-core lowering cannot inspect an imported core callable")
        };
        let candidate = authority
            .candidates
            .iter()
            .find(|candidate| candidate.reference == reference)
            .expect("an imported callable lookup returns a reference from its candidate set");
        ImportedCoreCallableCandidate {
            reference,
            target: candidate.target.clone(),
        }
    }

    pub(crate) fn imported_core_callable_by_declaration(
        &self,
        declaration: hir::DefaultCallableDeclarationV1,
    ) -> Option<hir::ImportedCorePreludeRef> {
        let hir::DefaultCallableDeclarationV1::Function(declaration) = declaration else {
            return None;
        };
        let CoreLoweringAuthority::Imported(authority) = &self.core else {
            return None;
        };
        authority.candidates.iter().find_map(|candidate| {
            matches!(
                candidate.target.definition(),
                hir::CoreCallableDefinitionV1::Function(id) if id == declaration
            )
            .then_some(candidate.reference)
        })
    }

    pub(crate) fn select_imported_core_callable(
        &mut self,
        reference: hir::ImportedCorePreludeRef,
        span: Span,
    ) -> Option<hir::ImportedCoreCallableUseId> {
        let selected = {
            let CoreLoweringAuthority::Imported(authority) = &mut self.core else {
                panic!("defined-core lowering cannot select an imported core callable")
            };
            authority.selection.select(reference)
        };
        let selected = match selected {
            Ok(hir::SelectedImportedCoreId::Callable(reference)) => reference,
            Ok(hir::SelectedImportedCoreId::Type(_) | hir::SelectedImportedCoreId::Value(_)) => {
                panic!("a callable prelude candidate selects a callable reference")
            }
            Err(error) => {
                self.error(span, error.to_string());
                return None;
            }
        };
        if let Some((id, _)) = self
            .imported_core_callables
            .iter()
            .find(|(_, use_)| use_.reference() == selected)
        {
            return Some(id);
        }
        Some(
            self.imported_core_callables
                .alloc(hir::ImportedCoreCallableUse::new(selected)),
        )
    }

    pub(crate) fn imported_signature_type(
        &mut self,
        signature: &SignatureTypeKey,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        match signature {
            SignatureTypeKey::Nominal(identity) => {
                let CoreLoweringAuthority::Imported(authority) = &self.core else {
                    panic!("defined-core lowering cannot map an imported signature")
                };
                let fundamental = authority.protocols.fundamental_types();
                if *identity == fundamental.unit().persistent() {
                    Ok(self.unit)
                } else if let Some(kind) = hir::IntegerKind::ALL
                    .into_iter()
                    .find(|kind| *identity == fundamental.integer(*kind).persistent())
                {
                    Ok(self.integer_type(kind))
                } else if *identity == fundamental.boolean().persistent() {
                    Ok(self.boolean)
                } else if *identity == fundamental.string().persistent() {
                    Ok(self.string)
                } else if *identity
                    == scoop_identity::CoreBuiltinNominal::Any
                        .identity_record()
                        .id()
                {
                    Ok(self.any)
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

pub(crate) fn imported_signature_subtype(
    left: &SignatureTypeKey,
    right: &SignatureTypeKey,
) -> bool {
    left == right
        || matches!(
            right,
            SignatureTypeKey::Nominal(identity)
                if *identity
                    == scoop_identity::CoreBuiltinNominal::Any
                        .identity_record()
                        .id()
        )
}
