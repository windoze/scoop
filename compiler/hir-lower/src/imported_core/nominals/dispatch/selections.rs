//! Substitute nominal binders once when importing interface table selections.

use std::collections::BTreeMap;

use super::*;

impl Lowerer {
    pub(super) fn imported_dispatch_receiver(
        &mut self,
        source: &SignatureTypeKey,
        receiver: hir::TypeId,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let bindings = self
            .imported_owner_arguments(receiver)
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                (
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    *argument,
                )
            })
            .collect();
        self.imported_signature_type_with_bindings(source, &bindings)
    }

    pub(in crate::imported_core) fn imported_interface_dispatch_selections<'a>(
        &mut self,
        receiver: hir::TypeId,
        declaration: &'a hir::ImportedNominalDeclaration,
    ) -> Result<
        BTreeMap<
            (hir::TypeId, scoop_identity::PersistentDispatchSlotId),
            &'a hir::NominalDispatchSelectionV1,
        >,
        ImportedSignatureTypeError,
    > {
        let bindings = self
            .imported_owner_arguments(receiver)
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                (
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    *argument,
                )
            })
            .collect();
        let mut selections = BTreeMap::new();
        for selection in declaration
            .interface
            .declaration_details()
            .declared_dispatch_selections()
        {
            let hir::NominalDispatchSelectionRoleV1::Interface { interface } = selection.role()
            else {
                continue;
            };
            let interface = self.imported_signature_type_with_bindings(interface, &bindings)?;
            if let Some(previous) = selections.insert((interface, selection.slot()), selection)
                && (previous.selection() != selection.selection()
                    || self
                        .imported_signature_type_with_bindings(previous.receiver(), &bindings)?
                        != self.imported_signature_type_with_bindings(
                            selection.receiver(),
                            &bindings,
                        )?)
            {
                return Err(ImportedSignatureTypeError::Structural);
            }
        }
        Ok(selections)
    }
}
