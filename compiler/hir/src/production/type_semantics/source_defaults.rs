//! Complete nominal parameter/default production, independent of candidate tables.
use super::nested_sources::inventory;
use crate::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::HashMap;
mod errors;
use NominalDefaultSourceProductionError as Error;
pub use errors::NominalDefaultSourceProductionError;

/// Complete source bodies and parameter facts; profiles and semantic replay are separate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalDefaultSourceProductionV1 {
    parameters: CanonicalNominalSourceParameterProtocolsV1,
    templates: CanonicalDefaultSourceTemplatesV1,
}
impl NominalDefaultSourceProductionV1 {
    pub fn from_dependency_hir(
        output: &DependencyHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        produce(&output.output().export, meter, |owner, position, meter| {
            DefaultSourceBodyProductionV1::from_dependency_hir(output, owner, position, meter)
                .map_err(Error::Body)?
                .into_source_template(meter)
                .map_err(Error::Template)
        })
    }
    pub fn from_export_hir(
        output: &ExportHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        produce(output, meter, |owner, position, meter| {
            DefaultSourceBodyProductionV1::from_export_hir(output.module(), owner, position, meter)
                .map_err(Error::Body)?
                .into_source_template(meter)
                .map_err(Error::Template)
        })
    }
    pub const fn parameters(&self) -> &CanonicalNominalSourceParameterProtocolsV1 {
        &self.parameters
    }
    pub const fn templates(&self) -> &CanonicalDefaultSourceTemplatesV1 {
        &self.templates
    }
    pub fn into_parts(
        self,
    ) -> (
        CanonicalNominalSourceParameterProtocolsV1,
        CanonicalDefaultSourceTemplatesV1,
    ) {
        (self.parameters, self.templates)
    }
}

fn produce(
    output: &ExportHirOutput,
    meter: &mut BudgetMeter,
    mut body: impl FnMut(
        ExportParameterOwner,
        u32,
        &mut BudgetMeter,
    ) -> Result<DefaultSourceTemplateV1, Error>,
) -> Result<NominalDefaultSourceProductionV1, Error> {
    let path = WirePath::root();
    meter
        .check_semantic_depth(1, &path)
        .map_err(Error::Resource)?;
    meter.charge_nodes(1, &path).map_err(Error::Resource)?;
    let roots =
        CanonicalSourceNominalIdsV1::from_export_hir(output, meter).map_err(Error::Sources)?;
    let nominals = CanonicalNominalSourceContractsV1::from_export_hir(output, &roots, meter)
        .map_err(Error::Sources)?;
    let required = inventory::collect(nominals.records().iter(), meter)
        .map_err(Error::Sources)?
        .protocols(meter)
        .map_err(Error::Sources)?;
    let parameters =
        CanonicalNominalSourceParameterProtocolsV1::from_export_hir(output, &required, meter)
            .map_err(Error::Sources)?;
    let owners = local_owners(output.module(), &parameters, meter)?;
    let mut records = Vec::new();
    for protocol in parameters.records() {
        meter.charge_work(1, &path).map_err(Error::Resource)?;
        let local = *owners
            .get(&protocol.owner())
            .ok_or(Error::MissingOwner(protocol.owner()))?;
        for (position, parameter) in protocol.parameters().iter().enumerate() {
            meter.charge_work(1, &path).map_err(Error::Resource)?;
            if matches!(
                parameter.calling_kind(),
                ProtectedParameterCallingKindV1::Required
                    | ProtectedParameterCallingKindV1::VarargEmpty
            ) {
                continue;
            }
            let position =
                u32::try_from(position).map_err(|_| Error::PositionOverflow(protocol.owner()))?;
            meter
                .check_table_entries(records.len() as u64 + 1, &path)
                .map_err(Error::Resource)?;
            meter
                .charge_owned_bytes(std::mem::size_of::<DefaultSourceTemplateV1>() as u64, &path)
                .map_err(Error::Resource)?;
            meter
                .try_reserve_collection_slots(&mut records, 1, &path)
                .map_err(Error::Resource)?;
            records.push(body(local, position, meter)?);
        }
    }
    let templates =
        CanonicalDefaultSourceTemplatesV1::try_new(records, meter).map_err(Error::Table)?;
    templates
        .validate_parameter_coverage(&parameters, meter)
        .map_err(Error::Coverage)?;
    Ok(NominalDefaultSourceProductionV1 {
        parameters,
        templates,
    })
}

fn local_owners(
    export: &ExportHir,
    parameters: &CanonicalNominalSourceParameterProtocolsV1,
    meter: &mut BudgetMeter,
) -> Result<HashMap<CallableTemplateOrigin, ExportParameterOwner>, Error> {
    let path = WirePath::root();
    meter
        .check_table_entries(export.source_parameter_interfaces.len() as u64, &path)
        .map_err(Error::Resource)?;
    meter
        .charge_owned_bytes(
            (parameters.records().len() as u64)
                .saturating_mul(
                    std::mem::size_of::<(CallableTemplateOrigin, ExportParameterOwner)>() as u64,
                ),
            &path,
        )
        .map_err(Error::Resource)?;
    let mut owners = HashMap::new();
    meter
        .try_reserve_map_slots(&mut owners, parameters.records().len(), &path)
        .map_err(Error::Resource)?;
    let probes = u64::from(parameters.records().len().max(1).ilog2()) + 1;
    for source in &export.source_parameter_interfaces {
        meter
            .charge_work(probes.saturating_mul(64), &path)
            .map_err(Error::Resource)?;
        let Some((declaration, _)) =
            super::nominal_parameters::owners::identity(export, source.owner)
        else {
            continue;
        };
        if parameters.get(declaration).is_none() {
            continue;
        }
        if owners.insert(declaration, source.owner).is_some() {
            return Err(Error::DuplicateOwner(declaration));
        }
    }
    Ok(owners)
}
