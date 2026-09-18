//! Imported-core prelude candidates and committed HIR uses.

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{
    BindingNamespace, ConeIdentity, NominalDeclarationOwner, PersistentTypeId, SignatureTypeKey,
};

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
            .callable_candidates
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
            .callable_candidates
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
        authority.callable_candidates.iter().find_map(|candidate| {
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

    /// Retains the exact trusted-core prelude route used by a type name while
    /// resolving one or more source type aliases. The route is attached to
    /// every active alias so a cached inner alias still contributes its
    /// foreign leaves to an enclosing expanded alias signature.
    pub(crate) fn retain_core_alias_target_binding(
        &mut self,
        name: &str,
        declaration: PersistentTypeId,
        span: Span,
    ) -> bool {
        if self.type_alias_resolution_stack.is_empty() {
            return true;
        }
        let binding = match &self.core {
            CoreLoweringAuthority::Defined => return true,
            CoreLoweringAuthority::Imported(authority) => authority
                .type_bindings
                .iter()
                .find(|candidate| {
                    candidate.name == name
                        && candidate.definition == hir::CoreTypeDefinitionV1::Type(declaration)
                })
                .cloned(),
        };
        let Some(binding) = binding else {
            self.error(
                span,
                format!("trusted core prelude does not expose the canonical type binding `{name}`"),
            );
            return false;
        };
        let route = hir::ReexportRouteV1::try_new(
            ConeIdentity::CORE,
            vec![hir::ReexportRouteHopV1::new(
                ConeIdentity::CORE,
                binding.binding,
            )],
        )
        .expect("one validated trusted-core prelude binding forms a direct route");
        let witness = hir::ExternalHirBindingWitnessUse::new(
            hir::ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(declaration)),
            hir::ExternalHirBindingWitnessRole::AliasTarget,
            hir::DependencyBindingWitnessV1::new(route),
        );
        for alias in self.type_alias_resolution_stack.iter().copied() {
            self.type_alias_binding_witnesses
                .entry(alias)
                .or_default()
                .push(witness.clone());
        }
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
