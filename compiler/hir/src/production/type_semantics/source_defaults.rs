//! Complete nominal parameter/default production, independent of candidate tables.
use super::nested_sources::inventory;
use crate::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::WirePath;
use std::collections::HashMap;
mod errors;
pub(super) mod profile;
use NominalDefaultSourceProductionError as Error;
pub use errors::NominalDefaultSourceProductionError;

/// Complete source bodies, parameter facts and profiles; semantic replay is separate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalDefaultSourceProductionV1 {
    parameters: CanonicalNominalSourceParameterProtocolsV1,
    templates: CanonicalDefaultSourceTemplatesV1,
    profiles: CanonicalDefaultSourceProfilesV1,
}
impl NominalDefaultSourceProductionV1 {
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        produce(&output.output().export, |owner, position| {
            DefaultSourceBodyProductionV1::from_dependency_hir(output, owner, position)
                .map_err(Error::Body)?
                .into_source_template()
                .map_err(Error::Template)
        })
    }
    pub fn from_export_hir(output: &ExportHirOutput) -> Result<Self, Error> {
        produce(output, |owner, position| {
            DefaultSourceBodyProductionV1::from_export_hir(output.module(), owner, position)
                .map_err(Error::Body)?
                .into_source_template()
                .map_err(Error::Template)
        })
    }
    pub const fn parameters(&self) -> &CanonicalNominalSourceParameterProtocolsV1 {
        &self.parameters
    }
    pub const fn templates(&self) -> &CanonicalDefaultSourceTemplatesV1 {
        &self.templates
    }
    pub const fn profiles(&self) -> &CanonicalDefaultSourceProfilesV1 {
        &self.profiles
    }
    pub fn into_parts(
        self,
    ) -> (
        CanonicalNominalSourceParameterProtocolsV1,
        CanonicalDefaultSourceTemplatesV1,
        CanonicalDefaultSourceProfilesV1,
    ) {
        (self.parameters, self.templates, self.profiles)
    }
}

fn produce(
    output: &ExportHirOutput,

    mut body: impl FnMut(ExportParameterOwner, u32) -> Result<DefaultSourceTemplateV1, Error>,
) -> Result<NominalDefaultSourceProductionV1, Error> {
    let path = WirePath::root();

    let roots = CanonicalSourceNominalIdsV1::from_export_hir(output).map_err(Error::Sources)?;
    let nominals = CanonicalNominalSourceContractsV1::from_export_hir(output, &roots)
        .map_err(Error::Sources)?;
    let required = inventory::collect(nominals.records().iter())
        .map_err(Error::Sources)?
        .protocols()
        .map_err(Error::Sources)?;
    let parameters = CanonicalNominalSourceParameterProtocolsV1::from_export_hir(output, &required)
        .map_err(Error::Sources)?;
    let owners = local_owners(output.module(), parameters.records().len(), |owner| {
        parameters.get(owner).is_some()
    })?;
    let mut records = Vec::new();
    let mut profiles = Vec::new();
    for protocol in parameters.records() {
        let local = *owners
            .get(&protocol.owner())
            .ok_or(Error::MissingOwner(protocol.owner()))?;
        for (position, parameter) in protocol.parameters().iter().enumerate() {
            if matches!(
                parameter.calling_kind(),
                ProtectedParameterCallingKindV1::Required
                    | ProtectedParameterCallingKindV1::VarargEmpty
            ) {
                continue;
            }
            let position =
                u32::try_from(position).map_err(|_| Error::PositionOverflow(protocol.owner()))?;

            scoop_wire::allocation::try_reserve(&mut records, 1, &path).map_err(Error::Resource)?;
            let source = body(local, position)?;
            let profile =
                profile::from_source(output.module(), local, &source).map_err(Error::Sources)?;

            scoop_wire::allocation::try_reserve(&mut profiles, 1, &path)
                .map_err(Error::Resource)?;
            profiles.push(DefaultSourceProfileV1::new(source.key(), profile));
            records.push(source);
        }
    }
    let templates = CanonicalDefaultSourceTemplatesV1::try_new(records).map_err(Error::Table)?;
    templates
        .validate_parameter_coverage(&parameters)
        .map_err(Error::Coverage)?;
    let profiles = CanonicalDefaultSourceProfilesV1::try_new(profiles).map_err(Error::Profiles)?;
    profiles
        .validate_template_coverage(&templates)
        .map_err(Error::Profiles)?;
    Ok(NominalDefaultSourceProductionV1 {
        parameters,
        templates,
        profiles,
    })
}

pub(super) fn local_owners(
    export: &ExportHir,
    count: usize,
    contains: impl Fn(CallableTemplateOrigin) -> bool,
) -> Result<HashMap<CallableTemplateOrigin, ExportParameterOwner>, Error> {
    let path = WirePath::root();

    let mut owners = HashMap::new();
    scoop_wire::allocation::try_reserve_map(&mut owners, count, &path).map_err(Error::Resource)?;

    for source in &export.source_parameter_interfaces {
        let Some((declaration, _)) =
            super::nominal_parameters::owners::identity(export, source.owner)
        else {
            continue;
        };
        if !contains(declaration) {
            continue;
        }
        if owners.insert(declaration, source.owner).is_some() {
            return Err(Error::DuplicateOwner(declaration));
        }
    }
    Ok(owners)
}
