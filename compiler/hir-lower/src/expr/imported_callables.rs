//! Shared winner commit for dependency callables.
//!
//! Name calls, property accessors and calls embedded in provider defaults all
//! converge here after their own source-level applicability checks. Keeping
//! arena interning in one place guarantees that one semantic selection has
//! one local HIR use id regardless of how it was reached.

use hir::ImportedCallableSource;
use scoop_hir as hir;

use super::MemberCallKind;
use crate::Lowerer;

impl Lowerer {
    pub(crate) fn select_imported_dependency_callable_use(
        &mut self,
        candidate: hir::ImportedDependencyCallableCandidate,
    ) -> Result<
        (
            hir::ImportedDependencyCallableUseId,
            std::sync::Arc<hir::DirectImportedTargetBinding>,
        ),
        hir::ImportedDependencySelectionError,
    > {
        let dispatch =
            self.imported_callable_dispatch(candidate.interface(), MemberCallKind::Ordinary)?;
        let binding = std::sync::Arc::new(candidate.binding().clone());
        let reference = self
            .dependencies
            .as_mut()
            .expect("ordinary lowering carries a dependency selection plan")
            .select_callable(candidate)?;
        Ok((
            self.intern_imported_dependency_callable_use(reference, dispatch),
            binding,
        ))
    }

    pub(crate) fn select_imported_callable_declaration_use_with_kind(
        &mut self,
        candidate: hir::ImportedCallableDeclaration,
        kind: MemberCallKind,
    ) -> Result<hir::ImportedDependencyCallableUseId, hir::ImportedDependencySelectionError> {
        let dispatch = self.imported_callable_dispatch(candidate.interface(), kind)?;
        self.select_imported_callable_with_dispatch(candidate, dispatch)
    }

    /// A dispatch table references the definition itself, including abstract traps.
    pub(crate) fn select_imported_callable_definition_use(
        &mut self,
        candidate: hir::ImportedCallableDeclaration,
    ) -> Result<hir::ImportedDependencyCallableUseId, hir::ImportedDependencySelectionError> {
        self.select_imported_callable_with_dispatch(
            candidate,
            hir::ImportedDependencyDispatch::Direct,
        )
    }

    fn select_imported_callable_with_dispatch(
        &mut self,
        candidate: hir::ImportedCallableDeclaration,
        dispatch: hir::ImportedDependencyDispatch,
    ) -> Result<hir::ImportedDependencyCallableUseId, hir::ImportedDependencySelectionError> {
        let reference = self
            .dependencies
            .as_mut()
            .expect("ordinary lowering carries a dependency selection plan")
            .select_declared_callable(candidate)?;
        Ok(self.intern_imported_dependency_callable_use(reference, dispatch))
    }

    fn imported_callable_dispatch(
        &mut self,
        callable: &hir::CallableDeclarationRecordV1,
        kind: MemberCallKind,
    ) -> Result<hir::ImportedDependencyDispatch, hir::ImportedDependencySelectionError> {
        use hir::ImportedDependencyDispatch as Dispatch;
        let invalid = || hir::ImportedDependencySelectionError::InvalidDispatch {
            declaration: callable.declaration(),
        };
        if kind == MemberCallKind::DirectSuper {
            return if callable.modality() == hir::CallableModalityV1::Abstract {
                Err(invalid())
            } else {
                Ok(Dispatch::Direct)
            };
        }
        if callable.modality() == hir::CallableModalityV1::Final {
            return Ok(Dispatch::Direct);
        }
        let hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::Concrete(owner)) =
            callable.owner()
        else {
            return Err(invalid());
        };
        let ty = self
            .imported_signature_type(&scoop_identity::SignatureTypeKey::Nominal(owner))
            .map_err(|_| invalid())?;
        match &self.types[ty] {
            hir::Type::ImportedClass(class) => {
                let position = class
                    .virtual_methods
                    .iter()
                    .position(|method| callable.slot_relations().values().contains(&method.slot))
                    .ok_or_else(invalid)?;
                Ok(Dispatch::Virtual {
                    slot: u32::try_from(position).map_err(|_| invalid())?,
                })
            }
            hir::Type::ImportedInterface(interface) => {
                let position = interface
                    .methods
                    .iter()
                    .position(|method| method.declaration.declaration() == callable.declaration())
                    .ok_or_else(invalid)?;
                Ok(Dispatch::Interface {
                    interface: owner,
                    slot: u32::try_from(position).map_err(|_| invalid())?,
                })
            }
            _ => Err(invalid()),
        }
    }

    fn intern_imported_dependency_callable_use(
        &mut self,
        reference: hir::ImportedDependencyCallableRef,
        dispatch: hir::ImportedDependencyDispatch,
    ) -> hir::ImportedDependencyCallableUseId {
        let existing = self
            .imported_dependency_callables
            .iter()
            .find_map(|(id, use_)| {
                (use_.reference() == reference && use_.dispatch() == dispatch).then_some(id)
            });
        match existing {
            Some(existing) => existing,
            None => self
                .imported_dependency_callables
                .alloc(hir::ImportedDependencyCallableUse::new(reference, dispatch)),
        }
    }
}
