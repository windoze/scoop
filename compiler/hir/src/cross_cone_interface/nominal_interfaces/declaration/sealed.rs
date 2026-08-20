//! Closed inheritance relations share the ordinary declaration inventory boundary.

use super::*;
use scoop_identity::{DeclarationName, DefinitionOwnerAtom, SignatureTypeKey};
use std::collections::BTreeMap;

impl CanonicalNominalInterfacesV1 {
    pub(super) fn validate_sealed_families(
        &self,
        foundation: &crate::CanonicalHirFoundation,
    ) -> Result<(), NominalDeclarationInventoryError> {
        if !self
            .all_records()
            .any(|record| record.declaration_details().sealed_subtypes().is_some())
        {
            return Ok(());
        }
        let keys = foundation
            .type_source_nominal_records()
            .iter()
            .map(|record| (SourceNominalId::Concrete(record.id()), record.key()))
            .chain(
                foundation
                    .type_source_generic_records()
                    .iter()
                    .map(|record| (SourceNominalId::GenericTemplate(record.id()), record.key())),
            )
            .collect::<BTreeMap<_, _>>();
        for root in self.all_records() {
            let Some(subtypes) = root.declaration_details().sealed_subtypes() else {
                continue;
            };
            let owner = root.declaration();
            for &subtype in subtypes.values() {
                let error = |reason| NominalDeclarationInventoryError::SealedFamily {
                    owner,
                    subtype,
                    reason,
                };
                let child = self
                    .declaration(subtype)
                    .ok_or_else(|| error("missing subtype declaration"))?;
                let parent_key = keys
                    .get(&owner)
                    .ok_or_else(|| error("missing sealed declaration key"))?;
                let child_key = keys
                    .get(&subtype)
                    .ok_or_else(|| error("missing subtype declaration key"))?;
                if parent_key.origin() != child_key.origin()
                    || parent_key.package() != child_key.package()
                    || !matches!(child_key.name(), DeclarationName::Named(_))
                    || !child_key.owners().owners().iter().all(|owner| {
                        matches!(
                            owner,
                            DefinitionOwnerAtom::Type(_) | DefinitionOwnerAtom::GenericType(_)
                        )
                    })
                {
                    return Err(error(
                        "subtype must be named, non-local and in the same origin Cone and package",
                    ));
                }
                if !child
                    .exact_supertypes()
                    .values()
                    .iter()
                    .any(|parent| signature_owner(parent) == Some(owner))
                {
                    return Err(error(
                        "subtype does not directly inherit the sealed declaration",
                    ));
                }
            }
        }
        for child in self.all_records() {
            for parent in child
                .exact_supertypes()
                .values()
                .iter()
                .filter_map(signature_owner)
            {
                if let Some(subtypes) = self
                    .declaration(parent)
                    .and_then(|parent| parent.declaration_details().sealed_subtypes())
                    && subtypes
                        .values()
                        .binary_search(&child.declaration())
                        .is_err()
                {
                    return Err(NominalDeclarationInventoryError::SealedFamily {
                        owner: parent,
                        subtype: child.declaration(),
                        reason: "sealed family omits a direct subtype",
                    });
                }
            }
        }
        Ok(())
    }
}

fn signature_owner(ty: &SignatureTypeKey) -> Option<SourceNominalId> {
    match ty {
        SignatureTypeKey::Nominal(id) => Some(SourceNominalId::Concrete(*id)),
        SignatureTypeKey::NominalApplication { origin, .. } => {
            Some(SourceNominalId::GenericTemplate(*origin))
        }
        _ => None,
    }
}
