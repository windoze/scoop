use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_resources::{self as resources, invalid, resource, work};
use crate::production::signatures::HirInterfaceSignatureProjector;
use crate::*;
use scoop_identity::{CallableTemplateOrigin, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeSet;

mod owners;
mod parameters;
mod signatures;

impl CanonicalNominalSourceParameterProtocolsV1 {
    /// Projects declaration-side parameter protocols independently of lookup,
    /// default bodies and executable materializations.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        required: &BTreeSet<CallableTemplateOrigin>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path).map_err(resource)?;
        meter
            .check_table_entries(required.len() as u64, &path)
            .map_err(resource)?;
        meter
            .charge_collection_slots(required.len() as u64, &path)
            .map_err(resource)?;
        meter
            .charge_work(required.len() as u64, &path)
            .map_err(resource)?;
        let records = project(output.module(), required.clone(), meter)?;
        Self::try_new(records, meter).map_err(Error::SourceInventory)
    }
}

pub(super) fn project(
    export: &ExportHir,
    required: BTreeSet<CallableTemplateOrigin>,
    meter: &mut BudgetMeter,
) -> Result<Vec<NominalSourceParameterProtocolV1>, Error> {
    let path = WirePath::root();
    meter.check_semantic_depth(1, &path).map_err(resource)?;
    meter
        .check_table_entries(required.len() as u64, &path)
        .map_err(resource)?;
    for owner in &required {
        work(meter, 1)?;
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
    meter
        .charge_collection_slots(required.len() as u64, &path)
        .map_err(resource)?;
    meter
        .charge_work(required.len() as u64, &path)
        .map_err(resource)?;
    let mut remaining = required.clone();
    let signatures = HirInterfaceSignatureProjector::new(export);
    let mut records = Vec::new();
    meter
        .try_reserve_collection_slots(&mut records, required.len(), &path)
        .map_err(resource)?;
    for interface in &export.source_parameter_interfaces {
        work(meter, required.len())?;
        let Some((declaration, key)) = owners::identity(export, interface.owner) else {
            continue;
        };
        if !required.contains(&declaration) {
            continue;
        }
        work(meter, remaining.len())?;
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
        let binders = owners::binders(export, &signatures, interface.owner, meter)?;
        let expected =
            signatures::expected(export, &signatures, interface.owner, key, &binders, meter)?;
        records.push(parameters::project(
            export,
            &signatures,
            declaration,
            &expected,
            interface,
            &binders,
            meter,
        )?);
    }
    if !remaining.is_empty() {
        return Err(invalid(
            "required nominal declaration has no source parameter interface",
        ));
    }
    Ok(records)
}
