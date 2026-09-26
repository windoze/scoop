use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_errors::{invalid, resource};
use crate::production::signatures::HirInterfaceSignatureProjector;
use crate::*;
use scoop_identity::{CallableTemplateOrigin, SourceDeclarationKey};
use scoop_wire::WirePath;
use std::collections::BTreeSet;

pub(super) mod owners;
mod parameters;
mod signatures;

pub(super) fn project(
    export: &ExportHir,
    required: BTreeSet<CallableTemplateOrigin>,
) -> Result<CanonicalProtectedCallableSourceInterfacesV1, Error> {
    let path = WirePath::root();

    for owner in &required {
        if !matches!(
            owner,
            CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Constructor(_)
                | CallableTemplateOrigin::VariantConstructor(_)
        ) {
            return Err(invalid(
                "nominal parameter source has another declaration role",
            ));
        }
    }

    let mut remaining = required.clone();
    let signatures = HirInterfaceSignatureProjector::new(export);
    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, required.len(), &path).map_err(resource)?;
    for interface in &export.source_parameter_interfaces {
        let Some((declaration, key)) = owners::identity(export, interface.owner) else {
            continue;
        };
        if !required.contains(&declaration) {
            continue;
        }

        if !remaining.remove(&declaration) {
            return Err(invalid("duplicate sealed nominal parameter interface"));
        }
        if key.origin() != export.cone
            || matches!(interface.owner,
            ExportParameterOwner::Function(id) if export.functions[id].method.is_none())
        {
            return Err(invalid(
                "nominal parameter interface has no local nominal declaration",
            ));
        }
        let binders = owners::binders(export, &signatures, interface.owner)?;
        let expected = signatures::expected(export, &signatures, interface.owner, key, &binders)?;
        records.push(parameters::project(
            export,
            &signatures,
            declaration,
            &expected,
            interface,
            &binders,
        )?);
    }
    if !remaining.is_empty() {
        return Err(invalid(
            "required nominal declaration has no source parameter interface",
        ));
    }
    CanonicalProtectedCallableSourceInterfacesV1::try_new(records).map_err(invalid)
}
