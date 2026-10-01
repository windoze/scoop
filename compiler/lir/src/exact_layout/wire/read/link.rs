//! Resolve stored layout constituents without importing source declarations.

use std::collections::BTreeMap;

use scoop_identity::{LayoutKey, PersistentIdResolver, ValidatedIdentityGraph};

use super::*;
use crate::{ConeLirFoundation, LinkDataError, LirTargetProfile, link_data::link_error};

mod instance;
mod value;

pub(crate) type LinkLayouts = BTreeMap<PersistentLayoutId, ExactLayoutExportV1>;
type NominalFieldValue = (
    scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentFieldId,
        scoop_identity::FieldIdentityKey,
    >,
    std::sync::Arc<ExactValueLayoutV1>,
);

pub(super) struct LayoutReader<'a> {
    pub target: LirTargetProfile,
    pub identities: &'a mut ValidatedIdentityGraph,
    pub foundation: &'a ConeLirFoundation,
    pub layouts: &'a LinkLayouts,
}

impl DecodedExactLayoutExportV1 {
    pub(crate) fn link_id(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<PersistentLayoutId, LinkDataError> {
        identities.resolve(self.semantic.layout).map_err(link_error)
    }

    pub(crate) fn link_dependencies(
        &self,
        target: LirTargetProfile,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<Vec<PersistentLayoutId>, LinkDataError> {
        let mut dependencies = Vec::new();
        let mut field = |storage: &crate::DecodedFieldStorageV1| {
            field_layout(storage, target, identities).map(|id| dependencies.push(id))
        };
        match &self.semantic.body {
            RawBody::Value { representation, .. } => match representation {
                RawValue::Struct { fields, .. } => {
                    for value in fields {
                        field(&value.storage)?;
                    }
                }
                RawValue::Tuple(elements) => {
                    for value in elements {
                        field(value.link_storage())?;
                    }
                }
                RawValue::TaggedEnum { variants, .. } => {
                    for variant in variants {
                        for value in &variant.variant.fields {
                            field(&value.storage)?;
                        }
                    }
                }
                RawValue::NicheEnum { variants, .. } => {
                    for variant in variants {
                        for value in &variant.fields {
                            field(&value.storage)?;
                        }
                    }
                }
                RawValue::Scalar(_) | RawValue::QualifiedPointer(_) | RawValue::Unit => {
                    return Ok(dependencies);
                }
            },
            RawBody::Instance { representation, .. } => match representation {
                RawInstance::Class { base, declared, .. } => {
                    for value in declared {
                        field(&value.storage)?;
                    }
                    if let RawBase::Prefix { layout, .. } = base {
                        dependencies.push(identities.resolve(*layout).map_err(link_error)?);
                    }
                }
                RawInstance::Box { layout, .. } => {
                    dependencies.push(identities.resolve(*layout).map_err(link_error)?);
                }
                RawInstance::InlineArray { exact, .. } => {
                    let exact = identities.resolve(*exact).map_err(link_error)?;
                    dependencies.push(value_layout_id(target, exact)?);
                }
                RawInstance::InlineBytes | RawInstance::AbstractReference => {
                    return Ok(dependencies);
                }
            },
        }
        dependencies.sort_unstable();
        dependencies.dedup();
        Ok(dependencies)
    }

    pub(crate) fn read_link(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
        layouts: &LinkLayouts,
    ) -> Result<ExactLayoutExportV1, LinkDataError> {
        let exact = identities
            .resolve(self.semantic.exact)
            .map_err(link_error)?;
        let identity = ExactLayoutIdentityV1::from_foundation(
            target,
            identities.canonical_record(exact).map_err(link_error)?,
            self.semantic.role,
            foundation,
        )
        .map_err(link_error)?;
        let mut reader = LayoutReader {
            target,
            identities,
            foundation,
            layouts,
        };
        let expected = match &self.semantic.body {
            RawBody::Value { representation, .. } => reader.value(identity, representation)?.into(),
            RawBody::Instance { representation, .. } => {
                reader.instance(identity, representation)?.into()
            }
        };
        self.validate_against(&expected).map_err(link_error)
    }
}

pub(super) fn value_layout_id(
    target: LirTargetProfile,
    exact: PersistentExactTypeId,
) -> Result<PersistentLayoutId, LinkDataError> {
    PersistentLayoutId::from_key(&LayoutKey::new(
        exact,
        target.wire_id(),
        RepresentationRole::ManagedValue,
    ))
    .map_err(link_error)
}

fn field_layout(
    field: &crate::DecodedFieldStorageV1,
    target: LirTargetProfile,
    identities: &mut ValidatedIdentityGraph,
) -> Result<PersistentLayoutId, LinkDataError> {
    match field.link_layout() {
        Some(layout) => identities.resolve(layout).map_err(link_error),
        None => value_layout_id(
            target,
            identities.resolve(field.link_exact()).map_err(link_error)?,
        ),
    }
}

impl LayoutReader<'_> {
    fn field_value(
        &mut self,
        field: &crate::DecodedFieldStorageV1,
    ) -> Result<std::sync::Arc<ExactValueLayoutV1>, LinkDataError> {
        let id = field_layout(field, self.target, self.identities)?;
        self.value_by_id(id)
    }

    fn value_by_id(
        &self,
        id: PersistentLayoutId,
    ) -> Result<std::sync::Arc<ExactValueLayoutV1>, LinkDataError> {
        self.layouts
            .get(&id)
            .and_then(ExactLayoutExportV1::value_handle)
            .ok_or_else(|| LinkDataError(format!("missing value layout {id}")))
    }

    fn nominal_fields(
        &mut self,
        fields: &[RawNominalField],
    ) -> Result<Vec<NominalFieldValue>, LinkDataError> {
        fields
            .iter()
            .map(|field| {
                let id = self.identities.resolve(field.id).map_err(link_error)?;
                Ok((
                    self.identities.canonical_record(id).map_err(link_error)?,
                    self.field_value(&field.storage)?,
                ))
            })
            .collect()
    }
}
