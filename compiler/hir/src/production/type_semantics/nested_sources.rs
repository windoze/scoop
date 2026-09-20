//! Recursive nominal candidates projected independently from sealed source HIR.
use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_resources::{invalid, resource, work};
use crate::production::nominal_interfaces::{NestedSourceNode, project_nested_sources};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId, PersistentPropertyId};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::{BTreeMap, BTreeSet};

mod assemble;
mod inventory;
mod protocols;
mod resources;

/// Source candidate and omission protocols. Default bodies, authority binding,
/// representation joins and executable capabilities are checked separately.
#[derive(Clone, Debug)]
pub struct NestedNominalSourceProductionV1 {
    record: NominalSupportNestedInterfaceV1,
    protocols: CanonicalProtectedCallableSourceInterfacesV1,
}
impl NestedNominalSourceProductionV1 {
    pub fn from_export_hir(
        output: &ExportHirOutput,
        root: SourceNominalId,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nodes = project_nested_sources(output, root, meter)?;
        let mut required = inventory::collect(&nodes, meter)?;
        let properties = super::inheritance::source_properties::project_nominal(
            output.module(),
            &required.properties(meter)?,
            meter,
        )?;
        required.accessors(&properties, meter)?;
        let protocol_owners = required.protocols(meter)?;
        let constructors =
            super::nominal_constructors::project(output.module(), required.constructors, meter)?;
        let callables =
            super::nominal_callables::project(output.module(), required.callables, meter)?;
        let protocols = protocols::project(output.module(), protocol_owners, meter)?;
        let mut assembly = assemble::Assembly::new(nodes, meter)?;
        for property in properties {
            let owner = property.owner();
            resources::boxed(&property, meter)?;
            assembly.push(
                owner,
                NestedSourceSupportV1::Property(Box::new(property)),
                meter,
            )?;
        }
        for constructor in constructors {
            let owner = constructor.payload().owner();
            resources::boxed(&constructor, meter)?;
            assembly.push(
                owner,
                NestedSourceSupportV1::Constructor(Box::new(constructor)),
                meter,
            )?;
        }
        for callable in callables {
            let owner = callable.payload().owner();
            resources::boxed(&callable, meter)?;
            assembly.push(
                owner,
                NestedSourceSupportV1::Callable(Box::new(callable)),
                meter,
            )?;
        }
        let record = assembly.finish(root, meter)?;
        Ok(Self { record, protocols })
    }
    pub const fn record(&self) -> &NominalSupportNestedInterfaceV1 {
        &self.record
    }
    pub const fn protocols(&self) -> &CanonicalProtectedCallableSourceInterfacesV1 {
        &self.protocols
    }
    pub fn into_parts(
        self,
    ) -> (
        NominalSupportNestedInterfaceV1,
        CanonicalProtectedCallableSourceInterfacesV1,
    ) {
        (self.record, self.protocols)
    }
}
