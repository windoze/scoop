//! Qualified dependency names use the same static bindings as imports.

use super::*;
use crate::imports::ImportLookupLayer;
use crate::imports::lookup::calls::{
    ExpressionQualifierLookup, ExpressionQualifierTarget, NamedCallBinding, NamedCallOrigin,
    NamedCallTarget,
};
use scoop_identity::BindingNamespace;

mod calls;
mod companions;
mod fields;
mod types;

#[derive(Clone, Copy)]
pub(in crate::expr) enum ImportedNominalQualifier {
    Applied(TypeId),
    Generic(scoop_identity::PersistentGenericTypeId),
}

impl ImportedNominalQualifier {
    fn applied(self) -> Option<TypeId> {
        match self {
            Self::Applied(ty) => Some(ty),
            Self::Generic(_) => None,
        }
    }
}

impl Lowerer {
    fn imported_qualifier_owner(
        &self,
        qualifier: ImportedNominalQualifier,
    ) -> hir::SourceNominalId {
        match qualifier {
            ImportedNominalQualifier::Applied(ty) => self
                .imported_nominal_owner(ty)
                .expect("a dependency qualifier retains its nominal declaration"),
            ImportedNominalQualifier::Generic(owner) => {
                hir::SourceNominalId::GenericTemplate(owner)
            }
        }
    }

    fn imported_static_bindings(
        &self,
        owner: hir::SourceNominalId,
        namespace: BindingNamespace,
        name: &str,
    ) -> Vec<hir::DirectImportedTargetBinding> {
        self.dependencies
            .as_ref()
            .map_or_else(Vec::new, |dependencies| {
                dependencies
                    .static_bindings(owner, namespace, name)
                    .to_vec()
            })
    }
}
