use std::collections::BTreeSet;

use scoop_identity::{
    CborIdentityRecord, EnumVariantFieldKey, EnumVariantIdentityKey, NominalDeclarationOwner,
    PersistentEnumVariantFieldId, PersistentEnumVariantId,
};

use super::*;
use crate::{
    EnumStorageGeometryV1, EnumVariantGeometryInputV1, FieldStorageV1, NullNicheKind,
    StorageGeometryV1,
};

#[derive(Clone, Copy, Debug)]
pub struct EnumLayoutFieldInputV1<'a> {
    pub field: &'a CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>,
    pub value: &'a ValueLayoutConstituentV1,
    pub pointer_kind: Option<NullNicheKind>,
}

#[derive(Clone, Copy, Debug)]
pub struct EnumLayoutVariantInputV1<'a> {
    pub variant: &'a CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>,
    pub fields: &'a [EnumLayoutFieldInputV1<'a>],
}

impl ExactValueLayoutV1 {
    /// Niche eligibility is derived from the same closed source-pointer
    /// representation shape; the caller cannot select a competing encoding.
    pub fn enumeration(
        identity: ExactLayoutIdentityV1,
        variants: &[EnumLayoutVariantInputV1<'_>],
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        validate_variants(&identity, variants)?;
        if let Some((index, kind, payload)) = niche(variants) {
            let roles: &[RepresentationRole] = match kind {
                NullNicheKind::Managed | NullNicheKind::Interface => {
                    &[RepresentationRole::ManagedValue]
                }
                NullNicheKind::Raw | NullNicheKind::Code => {
                    &[RepresentationRole::ManagedValue, RepresentationRole::CValue]
                }
            };
            require_roles(&identity, roles)?;
            let whole = geometry(payload)?;
            let mut placed = reserve(variants.len())?;
            for variant in variants {
                let mut fields = reserve(variant.fields.len())?;
                for field in variant.fields {
                    fields.push(EnumVariantFieldLayoutV1 {
                        field: field.field.id(),
                        storage: FieldStorageV1::within(field.value, 0, whole)?,
                        access_alignment: whole.alignment(),
                    });
                }
                placed.push(EnumVariantLayoutV1 {
                    variant: variant.variant.id(),
                    fields,
                });
            }
            return finish_value(
                identity,
                payload.storage().clone(),
                ValueRepresentation::NicheEnum(NicheEnumRepresentationLayoutV1 {
                    pointer_kind: kind,
                    variants: placed,
                    payload_variant: variants[index].variant.id(),
                }),
                foundation,
            );
        }
        require_roles(&identity, &[RepresentationRole::ManagedValue])?;
        let mut geometries = reserve(variants.len())?;
        for variant in variants {
            let mut fields = reserve(variant.fields.len())?;
            for field in variant.fields {
                fields.push(geometry(field.value)?);
            }
            geometries.push(fields);
        }
        let mut inputs = reserve(variants.len())?;
        for (variant, fields) in variants.iter().zip(&geometries) {
            inputs.push(EnumVariantGeometryInputV1 {
                fields,
                gc_free: variant
                    .fields
                    .iter()
                    .all(|field| !storage_scan(field.value.storage()).contains_reference()),
            });
        }
        let geometry = EnumStorageGeometryV1::tagged(identity.target(), &inputs)?;
        let mut placed = reserve(variants.len())?;
        for (variant, placement) in variants.iter().zip(geometry.variants()) {
            let mut fields = reserve(variant.fields.len())?;
            for (field, position) in variant.fields.iter().zip(placement.fields()) {
                fields.push(EnumVariantFieldLayoutV1 {
                    field: field.field.id(),
                    storage: FieldStorageV1::within(
                        field.value,
                        position.offset(),
                        geometry.storage(),
                    )?,
                    access_alignment: position.access_alignment(),
                });
            }
            placed.push(EnumVariantLayoutV1 {
                variant: variant.variant.id(),
                fields,
            });
        }
        let fields = placed
            .iter()
            .flat_map(|variant| variant.fields.iter().map(EnumVariantFieldLayoutV1::storage));

        let scan = FieldStorageV1::combined_scan(fields)?;
        let storage = ValueStorageLayoutV1::inline(
            geometry.storage().size(),
            geometry.storage().alignment().get(),
            scan,
        )?;
        finish_value(
            identity,
            storage,
            ValueRepresentation::TaggedEnum(TaggedEnumRepresentationLayoutV1 {
                geometry,
                variants: placed,
            }),
            foundation,
        )
    }
}

fn geometry(value: &ValueLayoutConstituentV1) -> Result<StorageGeometryV1, ExactLayoutReplayError> {
    Ok(StorageGeometryV1::new(
        value.target(),
        value.storage().byte_size(),
        value.storage().alignment().get(),
    )?)
}

fn validate_variants(
    identity: &ExactLayoutIdentityV1,
    variants: &[EnumLayoutVariantInputV1<'_>],
) -> Result<(), ExactLayoutReplayError> {
    let owner = nominal(identity.exact_key())?;
    if variants.is_empty() {
        return Err(ExactLayoutReplayError::EmptyEnum);
    }

    let mut seen = BTreeSet::new();
    for variant in variants {
        let key = variant.variant.key();
        let actual = key
            .source_owner()
            .or_else(|| key.generated_owner().map(NominalDeclarationOwner::Concrete));
        if actual != Some(owner) {
            return Err(ExactLayoutReplayError::VariantOwner);
        }
        if !seen.insert(variant.variant.id()) {
            return Err(ExactLayoutReplayError::DuplicateVariant);
        }

        let mut fields = BTreeSet::new();
        for (index, field) in variant.fields.iter().enumerate() {
            if field.field.key().variant() != variant.variant.id() {
                return Err(ExactLayoutReplayError::VariantFieldOwner);
            }
            if let scoop_identity::EnumVariantFieldSelector::Positional { declaration_index } =
                field.field.key().selector()
                && u64::from(*declaration_index) != index as u64
            {
                return Err(ExactLayoutReplayError::VariantFieldOwner);
            }
            if !fields.insert(field.field.id()) {
                return Err(ExactLayoutReplayError::DuplicateVariantField);
            }
            if field.value.target() != identity.target() {
                return Err(ExactLayoutReplayError::DependencyTarget);
            }
        }
    }
    Ok(())
}

fn niche<'a>(
    variants: &[EnumLayoutVariantInputV1<'a>],
) -> Option<(usize, NullNicheKind, &'a ValueLayoutConstituentV1)> {
    let [first, second] = variants else {
        return None;
    };
    let (index, payload) = match (first.fields, second.fields) {
        ([], [field]) => (1, field),
        ([field], []) => (0, field),
        _ => return None,
    };
    payload
        .pointer_kind
        .map(|kind| (index, kind, payload.value))
}
