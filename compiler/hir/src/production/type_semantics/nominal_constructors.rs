use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_errors::{invalid, resource};
use crate::production::{callable_interfaces, signatures::HirInterfaceSignatureProjector};
use crate::*;
use scoop_identity::{PersistentConstructorId, SourceDeclarationKey};
use scoop_wire::WirePath;
use std::collections::BTreeSet;

mod contract;

impl CanonicalNominalSourceConstructorsV1 {
    /// Projects independently required source constructors, including generic
    /// and restricted declarations, without constructing lookup candidates.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        required: &BTreeSet<PersistentConstructorId>,
    ) -> Result<Self, Error> {
        Self::try_new(project(output.module(), required.clone())?).map_err(Error::SourceInventory)
    }
}

struct Constructor<'a> {
    declaration: PersistentConstructorId,
    key: &'a SourceDeclarationKey,
    owner: &'a HirSourceNominalIdentity,
    binders: &'a [TypeParamDecl],
    parameters: ExportParameterOwner,
    result: TypeId,
    visibility: DeclaredVisibility,
    safety: Safety,
    gc_effect: GcEffect,
}

pub(super) fn project(
    export: &ExportHir,
    mut required: BTreeSet<PersistentConstructorId>,
) -> Result<Vec<NominalSupportConstructorInterfaceV1>, Error> {
    let path = WirePath::root();

    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, required.len(), &path).map_err(resource)?;
    for (id, source) in export.struct_constructors.iter() {
        let identity = &export.constructor_identities[id];
        if !required.remove(&identity.id()) {
            continue;
        }
        let owner = &export.structs[source.owner];
        records.push(contract::project(
            export,
            Constructor {
                declaration: identity.id(),
                key: identity.key(),
                owner: export.nominal_identities[source.owner]
                    .source()
                    .ok_or_else(|| invalid("constructor has no source nominal owner"))?,
                binders: &owner.type_params,
                parameters: ExportParameterOwner::StructConstructor(id),
                result: export.struct_applications[owner.self_application].canonical_type,
                visibility: source.access.declared,
                safety: source.safety,
                gc_effect: source.source_gc_effect(),
            },
        )?);
    }
    for (id, source) in export.class_constructors.iter() {
        let Some(identity) = export.constructor_identities[id].source_record() else {
            continue;
        };
        if !required.remove(&identity.id()) {
            continue;
        }
        let owner = &export.classes[source.owner];
        records.push(contract::project(
            export,
            Constructor {
                declaration: identity.id(),
                key: identity.key(),
                owner: export.nominal_identities[source.owner]
                    .source()
                    .ok_or_else(|| invalid("constructor has no source nominal owner"))?,
                binders: &owner.type_params,
                parameters: ExportParameterOwner::ClassConstructor(id),
                result: export.class_applications[owner.self_application].canonical_type,
                visibility: source.access.declared,
                safety: source.safety,
                gc_effect: GcEffect::Managed,
            },
        )?);
    }
    if !required.is_empty() {
        return Err(invalid(
            "required nominal source constructor has no sealed declaration",
        ));
    }
    Ok(records)
}
