//! Borrowed occurrence index over the independent source body.
use crate::*;
use scoop_wire::{BudgetMeter, WireError, WirePath};

mod descriptor;
mod query;
mod visit;
pub use descriptor::DefaultSourceNestedCallableDescriptorV1;
pub use query::DefaultSourceNestedCallableQueryError;

/// Raw source descriptors; indexing alone grants no semantic authority.
#[derive(Debug)]
pub struct DefaultSourceNestedCallablesV1<'a> {
    template: ProtectedDefaultTemplateKeyV1,
    occurrences: Vec<DefaultSourceNestedCallableOccurrenceV1<'a>>,
}
#[derive(Clone, Copy, Debug)]
pub struct DefaultSourceNestedCallableOccurrenceV1<'a> {
    site: DefaultNestedCallableSiteV1,
    descriptor: DefaultSourceNestedCallableDescriptorV1<'a>,
    definition_origin: &'a ExportDefinitionSourceV1,
}
impl DefaultSourceTemplateV1 {
    pub fn index_nested_callables(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceNestedCallablesV1<'_>, WireError> {
        let mut index = DefaultSourceNestedCallablesV1 {
            template: self.key(),
            occurrences: Vec::new(),
        };
        self.body().visit_direct_references(
            self.locals(),
            self.definition_origin(),
            &mut index,
            meter,
            path,
        )?;
        Ok(index)
    }
}
impl<'a> DefaultSourceNestedCallablesV1<'a> {
    pub const fn template(&self) -> ProtectedDefaultTemplateKeyV1 {
        self.template
    }
    pub fn occurrences(&self) -> &[DefaultSourceNestedCallableOccurrenceV1<'a>] {
        &self.occurrences
    }
}
impl<'a> DefaultSourceNestedCallableOccurrenceV1<'a> {
    pub const fn site(&self) -> DefaultNestedCallableSiteV1 {
        self.site
    }
    pub const fn descriptor(&self) -> DefaultSourceNestedCallableDescriptorV1<'a> {
        self.descriptor
    }
    pub const fn definition_origin(&self) -> &'a ExportDefinitionSourceV1 {
        self.definition_origin
    }
}
