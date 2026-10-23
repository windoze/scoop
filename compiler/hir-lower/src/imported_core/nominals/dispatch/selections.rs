//! Substitute nominal binders once when importing interface table selections.

use std::collections::BTreeMap;

use super::*;

pub(in crate::imported_core) type ImportedInterfaceSelections<'a> = BTreeMap<
    (
        hir::TypeId,
        Option<hir::ConditionalInterfaceApplication>,
        scoop_identity::PersistentDispatchSlotId,
    ),
    &'a hir::NominalDispatchSelectionV1,
>;

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
    ) -> Result<ImportedInterfaceSelections<'a>, ImportedSignatureTypeError> {
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
            .dispatch_selections()
            .records()
            .iter()
        {
            let Some(interface) = selection.role().interface() else {
                continue;
            };
            let condition = selection
                .role()
                .condition()
                .map(|condition| {
                    let owner_type =
                        self.imported_signature_type_with_bindings(&condition.owner, &bindings)?;
                    let owner = self
                        .imported_nominal_owner(owner_type)
                        .ok_or(ImportedSignatureTypeError::Structural)?;
                    if !self
                        .declaration_conditional_interfaces(owner)
                        .iter()
                        .any(|declaration| declaration.id() == condition.declaration)
                    {
                        return Err(ImportedSignatureTypeError::Structural);
                    }
                    Ok(hir::ConditionalInterfaceApplication {
                        declaration: condition.declaration,
                        owner,
                        arguments: self.imported_owner_arguments(owner_type).to_vec(),
                    })
                })
                .transpose()?;
            if condition
                .as_ref()
                .is_some_and(|condition| self.conditional_application_is_impossible(condition))
            {
                continue;
            }
            let interface = self.imported_signature_type_with_bindings(interface, &bindings)?;
            if let Some(previous) =
                selections.insert((interface, condition, selection.slot()), selection)
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
