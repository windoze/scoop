use super::*;
use crate::production::{callable_interfaces, signatures::HirInterfaceSignatureProjector};
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeSet;

mod accessors;
mod functions;
mod resources;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSourceProtectedCallablesV1, Error> {
    let mut required = BTreeSet::new();
    for owner in inventory.records() {
        work(meter, 1)?;
        for member in owner.protected_members().values() {
            work(meter, required.len())?;
            if let ProtectedDeclarationRefV1::Callable(declaration) = member {
                meter
                    .charge_collection_slots(1, &WirePath::root())
                    .map_err(resource)?;
                meter
                    .check_table_entries(required.len() as u64 + 1, &WirePath::root())
                    .map_err(resource)?;
                if !required.insert(declaration.declaration()) {
                    return Err(invalid(
                        "protected callable belongs to more than one source owner",
                    ));
                }
            }
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
    if !projection.required.is_empty() {
        return Err(invalid(
            "required protected callable has no sealed source declaration",
        ));
    }
    CanonicalInheritanceSourceProtectedCallablesV1::try_new(projection.records, projection.meter)
        .map_err(Error::SourceInventory)
}

struct Projection<'a, 'm> {
    export: &'a ExportHir,
    signatures: HirInterfaceSignatureProjector<'a>,
    required: BTreeSet<CallableTemplateOrigin>,
    records: Vec<ProtectedCallableInterfaceV1>,
    meter: &'m mut BudgetMeter,
}

impl Projection<'_, '_> {
    fn take(&mut self, declaration: CallableTemplateOrigin) -> Result<bool, Error> {
        work(self.meter, self.required.len())?;
        Ok(self.required.remove(&declaration))
    }

    fn access(
        &mut self,
        key: &SourceDeclarationKey,
        subject: DefinitionOriginSubject,
    ) -> Result<DeclarationAccessSourceV1, Error> {
        let path = WirePath::root();
        let owners = key.owners().owners().len() as u64;
        self.meter
            .check_semantic_depth(owners + 1, &path)
            .map_err(resource)?;
        self.meter
            .charge_collection_slots(owners, &path)
            .map_err(resource)?;
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
        super::super::nominals::declaration_access_for_subject(
            self.export,
            key,
            subject,
            DeclaredVisibilityV1::Protected,
        )
    }

    fn slots(&mut self, method: Method) -> Result<CanonicalProtectedSlotRefsV1, Error> {
        let slot = match method.dispatch {
            MethodDispatch::Direct => None,
            MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family) => {
                work(self.meter, self.export.functions.len())?;
                Some(
                    self.export
                        .dispatch_slot_identities
                        .get_virtual(family)
                        .ok_or_else(|| {
                            invalid("protected virtual family has no sealed slot identity")
                        })?
                        .id(),
                )
            }
            MethodDispatch::Interface(_) => {
                return Err(invalid("protected callable cannot be an interface member"));
            }
        };
        self.meter
            .charge_collection_slots(u64::from(slot.is_some()), &WirePath::root())
            .map_err(resource)?;
        CanonicalProtectedSlotRefsV1::try_new(slot.into_iter().collect()).map_err(invalid)
    }

    fn push(
        &mut self,
        declaration: CallableTemplateOrigin,
        access: DeclarationAccessSourceV1,
        payload: ProtectedCallablePayloadV1,
    ) -> Result<(), Error> {
        self.meter
            .check_table_entries(self.records.len() as u64 + 1, &WirePath::root())
            .map_err(resource)?;
        self.meter
            .try_reserve_collection_slots(&mut self.records, 1, &WirePath::root())
            .map_err(resource)?;
        self.records.push(
            ProtectedCallableInterfaceV1::try_new(declaration, access, payload).map_err(invalid)?,
        );
        Ok(())
    }
}

fn modality(modifier: MethodModifier) -> CallableModalityV1 {
    match modifier {
        MethodModifier::Final => CallableModalityV1::Final,
        MethodModifier::Open => CallableModalityV1::Open,
        MethodModifier::Abstract => CallableModalityV1::Abstract,
    }
}
fn owner(access: &DeclarationAccessSourceV1) -> Result<SourceNominalId, Error> {
    access
        .lexical_owners()
        .last()
        .copied()
        .ok_or_else(|| invalid("protected callable has no lexical nominal owner"))
}
fn work(meter: &mut BudgetMeter, length: usize) -> Result<(), Error> {
    meter
        .charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())
        .map_err(resource)
}
fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
fn invalid(reason: impl std::fmt::Display) -> Error {
    Error::InvalidSourceDeclaration(reason.to_string())
}
