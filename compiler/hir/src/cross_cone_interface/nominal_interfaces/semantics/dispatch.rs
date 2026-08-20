//! Check the new table applications in the nominal's own binder scope.

use super::*;
use crate::NominalDispatchSelectionRoleV1;

impl NominalInterfaceRecordV1 {
    pub(super) fn validate_dispatch_applications<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), NominalInterfaceSemanticValidationError<E>>
    where
        A: NominalInterfaceSemanticAuthority<E>,
    {
        let scope = self.type_parameters.signature_scope(None);
        let mut previous = None;
        let mut types = std::collections::BTreeMap::new();
        let mut receivers = std::collections::BTreeSet::new();
        for (index, selection) in self
            .details
            .dispatch_selections()
            .records()
            .iter()
            .enumerate()
        {
            if receivers.insert(selection.receiver()) {
                scope
                    .validate_signature_semantics(selection.receiver(), authority)
                    .map_err(
                        |error| NominalInterfaceSemanticValidationError::DispatchReceiver {
                            index,
                            error,
                        },
                    )?;
            }
            if previous == Some(selection.role()) {
                continue;
            }
            previous = Some(selection.role());
            match selection.role() {
                NominalDispatchSelectionRoleV1::ClassVtable => {
                    if !matches!(
                        self.kind,
                        PublicNominalKindV1::Class | PublicNominalKindV1::Object
                    ) {
                        return Err(NominalInterfaceSemanticValidationError::ClassDispatchRole(
                            self.kind,
                        ));
                    }
                }
                NominalDispatchSelectionRoleV1::Interface { interface } => {
                    let error =
                        |error| NominalInterfaceSemanticValidationError::DispatchApplication {
                            index,
                            error,
                        };
                    let kind = if let Some(kind) = types.get(interface) {
                        *kind
                    } else {
                        let shape = scope
                            .validate_nominal_signature_semantics(interface, authority)
                            .map_err(|source| {
                                error(ExactSupertypeSemanticError::Signature(source))
                            })?;
                        types.insert(interface, shape.kind());
                        shape.kind()
                    };
                    if kind != PublicNominalKindV1::Interface {
                        return Err(error(ExactSupertypeSemanticError::InvalidTargetKind(kind)));
                    }
                }
            }
        }
        Ok(())
    }
}
