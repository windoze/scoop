use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ExactTypeKey, PersistentExactTypeId, PersistentTypeId};

use super::{ConcreteNominal, NominalLocalId, identity, representation};
use crate::*;

use super::super::{
    CrossConeTypeSemanticsProductionError as Error, TypeSemanticsRepresentationEvidenceV1,
    TypeSemanticsSourceEvidenceV1,
};

pub(super) fn exact_type_keys(
    local: &LocalConcreteHir,
    generated: &BTreeMap<PersistentTypeId, scoop_identity::GeneratedNominalKey>,
) -> Result<BTreeMap<PersistentExactTypeId, ExactTypeKey>, Error> {
    let mut keys = local
        .types
        .iter()
        .map(|(ty, _)| {
            let record = &local.exact_type_identities[ty];
            (record.id(), record.key().clone())
        })
        .collect::<BTreeMap<_, _>>();
    for owner in generated.keys() {
        let key = ExactTypeKey::Nominal(*owner);
        let exact = PersistentExactTypeId::from_key(&key).map_err(|error| Error::InvalidTable {
            table: "generated exact-type identity",
            reason: error.to_string(),
        })?;
        keys.insert(exact, key);
    }
    Ok(keys)
}

pub(super) fn generated_nominal_keys(
    export: &ExportHir,
) -> Result<BTreeMap<PersistentTypeId, scoop_identity::GeneratedNominalKey>, Error> {
    let mut keys = BTreeMap::new();
    for local in all_nominals(export) {
        if let Some(record) = identity(export, local)?.generated() {
            keys.insert(record.id(), record.key().clone());
        }
    }
    Ok(keys)
}

pub(super) fn property_accessor_keys(
    export: &ExportHir,
) -> BTreeMap<scoop_identity::PersistentPropertyAccessorId, scoop_identity::PropertyAccessorKey> {
    export
        .property_getters
        .iter()
        .map(|(id, _)| export.property_accessor_identities[id].record())
        .chain(
            export
                .property_setters
                .iter()
                .map(|(id, _)| export.property_accessor_identities[id].record()),
        )
        .map(|record| (record.id(), *record.key()))
        .collect()
}

/// Reprojects representation authority directly from Export/LocalConcrete
/// HIR. This pass never receives candidate representation records.
pub(super) fn representation_evidence(
    export: &ExportHir,
    local: &LocalConcreteHir,
    nominals: &[ConcreteNominal<'_>],
    sources: &BTreeMap<SourceNominalId, TypeSemanticsSourceEvidenceV1>,
    public: &CanonicalNominalInterfacesV1,
) -> Result<BTreeMap<PersistentTypeId, TypeSemanticsRepresentationEvidenceV1>, Error> {
    let mut evidence = BTreeMap::new();
    for nominal in nominals {
        let source_id = SourceNominalId::Concrete(nominal.owner);
        let source = sources
            .get(&source_id)
            .ok_or(Error::MissingLocalSupport(nominal.exact))?;
        let shape = representation::shape(export, local, nominal)?;
        let public_value_shape = match nominal.local {
            NominalLocalId::Struct(_) | NominalLocalId::Enum(_) => public
                .get(source_id)
                .map(|interface| interface.source_shape().clone()),
            NominalLocalId::Class(_) | NominalLocalId::Interface(_) | NominalLocalId::Object(_) => {
                None
            }
        };
        let object_record = matches!(nominal.local, NominalLocalId::Object(_))
            .then(|| {
                NominalRepresentationSupportV1::try_new(
                    nominal.source.declaration(),
                    source.access.clone(),
                    shape.clone(),
                )
                .map_err(|error| Error::InvalidRepresentation {
                    declaration: source_id,
                    reason: error.to_string(),
                })
            })
            .transpose()?;
        evidence.insert(
            nominal.owner,
            TypeSemanticsRepresentationEvidenceV1 {
                shape,
                public_value_shape,
                object_record,
            },
        );
    }
    Ok(evidence)
}

pub(super) fn definition_sources(export: &ExportHir) -> BTreeSet<ExportDefinitionSourceV1> {
    export
        .export_definition_origins
        .records()
        .iter()
        .map(|record| ExportDefinitionSourceV1::new(record.origin().clone()))
        .collect()
}

pub(in crate::production::type_semantics) fn all_nominals(
    export: &ExportHir,
) -> impl Iterator<Item = NominalLocalId> + '_ {
    export
        .structs
        .iter()
        .map(|(id, _)| NominalLocalId::Struct(id))
        .chain(export.enums.iter().map(|(id, _)| NominalLocalId::Enum(id)))
        .chain(
            export
                .classes
                .iter()
                .map(|(id, _)| NominalLocalId::Class(id)),
        )
        .chain(
            export
                .interfaces
                .iter()
                .map(|(id, _)| NominalLocalId::Interface(id)),
        )
        .chain(
            export
                .objects
                .iter()
                .map(|(id, _)| NominalLocalId::Object(id)),
        )
}
