use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_resources::{self as resources, invalid, resource, work};
use super::nominals;
use crate::*;
use scoop_identity::{DefinitionOriginSubject as Subject, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::{BTreeMap, BTreeSet};
mod index;

impl CanonicalDefaultSourceAccessDeclarationsV1 {
    /// Projects independently demanded declarations and their full lexical owner
    /// closure. Foreign providers must project their own artifact sources.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        required: &BTreeSet<Subject>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path).map_err(resource)?;
        meter
            .check_table_entries(required.len() as u64, &path)
            .map_err(resource)?;
        meter
            .charge_work(required.len() as u64, &path)
            .map_err(resource)?;
        meter
            .charge_collection_slots(required.len() as u64, &path)
            .map_err(resource)?;
        for subject in required {
            DefaultSourceAccessDeclarationV1::validate_subject(*subject)
                .map_err(Error::SourceInventory)?;
        }
        let export = output.module();
        let index = index::collect(export, meter)?;
        let mut pending = required.clone();
        let mut records = BTreeMap::new();
        while let Some(subject) = pending.pop_first() {
            work(meter, pending.len())?;
            work(meter, records.len())?;
            if records.contains_key(&subject) {
                continue;
            }
            work(meter, index.len())?;
            let declaration = index.get(&subject).ok_or_else(|| {
                invalid(format!(
                    "required default access declaration has no local sealed source: {subject:?}"
                ))
            })?;
            let access = declaration.access(export, subject, meter)?;
            for owner in access.lexical_owners() {
                let owner = match owner {
                    SourceNominalId::Concrete(id) => Subject::Type(*id),
                    SourceNominalId::GenericTemplate(id) => Subject::GenericType(*id),
                };
                work(meter, records.len())?;
                work(meter, pending.len())?;
                if !records.contains_key(&owner) && !pending.contains(&owner) {
                    meter
                        .check_table_entries(pending.len() as u64 + 1, &path)
                        .map_err(resource)?;
                    meter.charge_collection_slots(1, &path).map_err(resource)?;
                    work(meter, pending.len())?;
                    pending.insert(owner);
                }
            }
            meter
                .check_table_entries(records.len() as u64 + 1, &path)
                .map_err(resource)?;
            meter.charge_collection_slots(1, &path).map_err(resource)?;
            let record = DefaultSourceAccessDeclarationV1::try_new(subject, access)
                .map_err(Error::SourceInventory)?;
            work(meter, records.len())?;
            records.insert(subject, record);
        }
        let mut output = Vec::new();
        meter
            .try_reserve_collection_slots(&mut output, records.len(), &path)
            .map_err(resource)?;
        meter
            .charge_work(records.len() as u64, &path)
            .map_err(resource)?;
        output.extend(records.into_values());
        Self::try_new(output, meter).map_err(Error::SourceInventory)
    }
}

struct Declaration<'a> {
    key: &'a SourceDeclarationKey,
    visibility: DeclaredVisibility,
}
impl Declaration<'_> {
    fn access(
        &self,
        export: &ExportHir,
        subject: Subject,
        meter: &mut BudgetMeter,
    ) -> Result<DeclarationAccessSourceV1, Error> {
        let path = WirePath::root();
        let count = self.key.owners().owners().len() as u64;
        meter
            .check_semantic_depth(count + 1, &path)
            .map_err(resource)?;
        meter.check_table_entries(count, &path).map_err(resource)?;
        // The result owns its owner chain; construction also checks uniqueness.
        meter
            .charge_collection_slots(count.saturating_mul(2), &path)
            .map_err(resource)?;
        meter
            .charge_work(
                count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
                &path,
            )
            .map_err(resource)?;
        let owners = nominals::lexical_owners(self.key)?;
        work(meter, export.export_definition_origins.records().len())?;
        let origin = export
            .export_definition_origins
            .get(subject)
            .ok_or(Error::MissingDefinitionOrigin(subject))?;
        resources::name(origin.origin().source().logical_path().as_str(), meter)?;
        DeclarationAccessSourceV1::try_new(
            self.visibility.into(),
            owners,
            ExportDefinitionSourceV1::new(origin.origin().clone()),
        )
        .map_err(invalid)
    }
}
