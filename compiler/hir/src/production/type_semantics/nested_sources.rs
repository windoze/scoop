//! Recursive nominal candidates projected independently from sealed source HIR.
use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_errors::{invalid, resource};
use crate::production::nominal_interfaces::{NestedSourceNode, project_nested_sources};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, PersistentConstructorId, PersistentPropertyId};
use scoop_wire::WirePath;
use std::collections::{BTreeMap, BTreeSet};

mod assemble;
pub(super) mod inventory;
mod protocols;
pub(super) mod resources;
pub(super) use protocols::project as project_protocols;

/// Source candidate and omission protocols. Default bodies, authority binding,
/// representation joins and executable capabilities are checked separately.
#[derive(Clone, Debug)]
pub struct NestedNominalSourceProductionV1 {
    record: NominalSupportNestedInterfaceV1,
    protocols: CanonicalProtectedCallableSourceInterfacesV1,
}
impl NestedNominalSourceProductionV1 {
    pub fn from_export_hir(output: &ExportHirOutput, root: SourceNominalId) -> Result<Self, Error> {
        let (record, owners) = project_record(output, root)?;
        let protocols = project_protocols(output.module(), owners)?;
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

pub(super) fn project_record(
    output: &ExportHirOutput,
    root: SourceNominalId,
) -> Result<
    (
        NominalSupportNestedInterfaceV1,
        BTreeSet<CallableTemplateOrigin>,
    ),
    Error,
> {
    let nodes = project_nested_sources(output, root)?;
    let mut required = inventory::collect(nodes.iter().map(|node| &node.contract))?;
    let properties = super::inheritance::source_properties::project_nominal(
        output.module(),
        &required.properties()?,
    )?;
    required.accessors(&properties)?;
    let protocol_owners = required.protocols()?;
    let constructors =
        super::nominal_constructors::project(output.module(), required.constructors)?;
    let callables = super::nominal_callables::project(output.module(), required.callables)?;
    let mut assembly = assemble::Assembly::new(nodes)?;
    for property in properties {
        let owner = property.owner();

        assembly.push(owner, NestedSourceSupportV1::Property(Box::new(property)))?;
    }
    for constructor in constructors {
        let owner = constructor.payload().owner();

        assembly.push(
            owner,
            NestedSourceSupportV1::Constructor(Box::new(constructor)),
        )?;
    }
    for callable in callables {
        let owner = callable.payload().owner();

        assembly.push(owner, NestedSourceSupportV1::Callable(Box::new(callable)))?;
    }
    let record = assembly.finish(root)?;
    Ok((record, protocol_owners))
}
