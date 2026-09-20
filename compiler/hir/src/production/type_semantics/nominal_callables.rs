use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_resources::{self as resources, invalid, resource, work};
use crate::production::{callable_interfaces, signatures::HirInterfaceSignatureProjector};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeSet;

mod accessors;
mod functions;
mod parameters;
mod relations;
mod variants;

impl CanonicalNominalSourceCallablesV1 {
    /// Projects independently required source methods, accessors and variants,
    /// including restricted and generic metadata, without candidate interfaces.
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
            .charge_work(required.len() as u64, &path)
            .map_err(resource)?;
        meter
            .charge_collection_slots(required.len() as u64, &path)
            .map_err(resource)?;
        let records = project(output.module(), required.clone(), meter)?;
        Self::try_new(records, meter).map_err(Error::SourceInventory)
    }
}

pub(super) fn project(
    export: &ExportHir,
    required: BTreeSet<CallableTemplateOrigin>,
    meter: &mut BudgetMeter,
) -> Result<Vec<NominalSupportCallableInterfaceV1>, Error> {
    let path = WirePath::root();
    meter.check_semantic_depth(1, &path).map_err(resource)?;
    meter
        .check_table_entries(required.len() as u64, &path)
        .map_err(resource)?;
    for declaration in &required {
        work(meter, 1)?;
        if !matches!(
            declaration,
            CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Accessor(_)
                | CallableTemplateOrigin::VariantConstructor(_)
        ) {
            return Err(invalid(
                "nominal callable source has another declaration role",
            ));
        }
    }
    let mut projection = Projection {
        export,
        signatures: HirInterfaceSignatureProjector::new(export),
        required,
        records: Vec::new(),
        meter,
    };
    projection.functions()?;
    projection.accessors()?;
    projection.variants()?;
    if !projection.required.is_empty() {
        return Err(invalid(
            "required nominal source callable has no sealed declaration",
        ));
    }
    Ok(projection.records)
}

struct Projection<'a, 'm> {
    export: &'a ExportHir,
    signatures: HirInterfaceSignatureProjector<'a>,
    required: BTreeSet<CallableTemplateOrigin>,
    records: Vec<NominalSupportCallableInterfaceV1>,
    meter: &'m mut BudgetMeter,
}
impl Projection<'_, '_> {
    fn take(&mut self, declaration: CallableTemplateOrigin) -> Result<bool, Error> {
        work(self.meter, self.required.len())?;
        Ok(self.required.remove(&declaration))
    }
    fn origin(
        &mut self,
        subject: DefinitionOriginSubject,
    ) -> Result<ExportDefinitionSourceV1, Error> {
        work(
            self.meter,
            self.export.export_definition_origins.records().len(),
        )?;
        let origin = self
            .export
            .export_definition_origins
            .get(subject)
            .ok_or(Error::MissingDefinitionOrigin(subject))?;
        resources::name(origin.origin().source().logical_path().as_str(), self.meter)?;
        Ok(ExportDefinitionSourceV1::new(origin.origin().clone()))
    }
    fn access(
        &mut self,
        key: &SourceDeclarationKey,
        subject: DefinitionOriginSubject,
        visibility: DeclaredVisibility,
    ) -> Result<DeclarationAccessSourceV1, Error> {
        if key.origin() != self.export.cone {
            return Err(invalid("nominal callable source belongs to another Cone"));
        }
        let path = WirePath::root();
        let count = key.owners().owners().len() as u64;
        self.meter
            .check_semantic_depth(count + 1, &path)
            .map_err(resource)?;
        self.meter
            .charge_collection_slots(count, &path)
            .map_err(resource)?;
        self.meter.charge_work(count, &path).map_err(resource)?;
        let owners = super::nominals::lexical_owners(key)?;
        let origin = self.origin(subject)?;
        DeclarationAccessSourceV1::try_new(visibility.into(), owners, origin).map_err(invalid)
    }
    fn push(
        &mut self,
        declaration: CallableTemplateOrigin,
        access: DeclarationAccessSourceV1,
        payload: NominalSourceCallablePayloadV1,
    ) -> Result<(), Error> {
        let path = WirePath::root();
        self.meter
            .check_table_entries(self.records.len() as u64 + 1, &path)
            .map_err(resource)?;
        self.meter
            .try_reserve_collection_slots(&mut self.records, 1, &path)
            .map_err(resource)?;
        self.records.push(
            NominalSupportCallableInterfaceV1::try_new(declaration, access, payload)
                .map_err(invalid)?,
        );
        Ok(())
    }
}

fn owner(access: &DeclarationAccessSourceV1) -> Result<SourceNominalId, Error> {
    access
        .lexical_owners()
        .last()
        .copied()
        .ok_or_else(|| invalid("nominal callable has no lexical nominal owner"))
}
