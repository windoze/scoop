use super::*;

#[derive(Clone)]
pub(super) struct ImportedCallableTarget {
    pub(super) entry: ImportedCallableEntry,
    pub(super) lowering_role: mir::MirCallableLoweringRoleV1,
    pub(super) signature: scoop_identity::ExactCallableSignature,
    pub(super) semantic_signature: scoop_identity::ExactCallableSignature,
}

#[derive(Clone)]
pub(super) enum ImportedCallableEntry {
    Scoop {
        callable: mir::ExternalCallableUseId,
        native_c: Option<ImportedCFunction>,
    },
    ReleaseNative(ImportedCFunction),
}

#[derive(Clone)]
pub(super) struct ImportedCFunction {
    pub(super) contract: scoop_identity::SourceNativeExternalContractRecord,
    pub(super) call_mode: scoop_identity::CAbiCallMode,
    pub(super) result: scoop_identity::ExternResult<hir::TypeId>,
}

impl ImportedCFunction {
    pub(super) fn new(
        module: &hir::Module,
        contract: scoop_identity::SourceNativeExternalContractRecord,
        call_mode: scoop_identity::CAbiCallMode,
        adaptation: scoop_identity::CResultAdaptation,
        exact: scoop_identity::PersistentExactTypeId,
    ) -> Result<Self, crate::current::CurrentConeMirLoweringError> {
        use crate::current::CurrentConeMirLoweringError as Error;
        use scoop_identity::{CResultAdaptation, ExternResult};
        let scoop = module
            .exact_type_identities
            .type_for_identity(exact)
            .ok_or(Error::MissingExternalSignatureType(exact))?;
        let result = match adaptation {
            CResultAdaptation::Direct => ExternResult::Direct(scoop),
            CResultAdaptation::CaptureErrno => {
                let hir::TypeKind::Tuple(elements) = &module.types[scoop].kind else {
                    return Err(Error::InvalidErrnoResultType(exact));
                };
                if elements.len() != 2
                    || !matches!(
                        module.types[elements[1]].kind,
                        hir::TypeKind::Integer(hir::IntegerKind::SIGNED_32)
                    )
                {
                    return Err(Error::InvalidErrnoResultType(exact));
                }
                ExternResult::CaptureErrno {
                    native: elements[0],
                    scoop,
                }
            }
        };
        Ok(Self {
            contract,
            call_mode,
            result,
        })
    }
}

impl ImportedCallableTarget {
    pub(super) fn external(&self) -> Option<mir::ExternalCallableUseId> {
        match self.entry {
            ImportedCallableEntry::Scoop { callable, .. } => Some(callable),
            ImportedCallableEntry::ReleaseNative(_) => None,
        }
    }

    pub(super) fn scoop_entry(&self) -> mir::ExternalCallableUseId {
        self.external()
            .expect("an ordinary use retains its Scoop entry")
    }

    pub(super) fn native_c(&self) -> Option<&ImportedCFunction> {
        match &self.entry {
            ImportedCallableEntry::Scoop { native_c, .. } => native_c.as_ref(),
            ImportedCallableEntry::ReleaseNative(contract) => Some(contract),
        }
    }
}
