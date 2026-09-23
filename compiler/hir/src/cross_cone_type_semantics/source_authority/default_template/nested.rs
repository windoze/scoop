//! Borrowed occurrence index over source and published default bodies.
use crate::*;
use scoop_wire::{BudgetMeter, WireError, WirePath};

mod attachment;
mod descriptor;
mod query;
mod visit;
pub use descriptor::DefaultSourceNestedCallableDescriptorV1;
pub use query::DefaultSourceNestedCallableQueryError;

/// Raw source descriptors; indexing alone grants no semantic authority.
#[derive(Debug)]
pub struct DefaultSourceNestedCallablesV1<'a, K = ProtectedDefaultTemplateKeyV1> {
    template: K,
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
        DefaultSourceNestedCallablesV1::from_body(
            self.key(),
            self.body(),
            self.locals(),
            self.definition_origin(),
            meter,
            path,
        )
    }
}
impl<'a, K: Copy> DefaultSourceNestedCallablesV1<'a, K> {
    pub const fn template(&self) -> K {
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

impl ExportDefaultTemplateV1 {
    pub fn index_nested_callables(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceNestedCallablesV1<'_, ExportDefaultTemplateKeyV1>, WireError> {
        DefaultSourceNestedCallablesV1::from_body(
            self.key(),
            self.body(),
            self.locals(),
            self.definition_origin(),
            meter,
            path,
        )
    }
}
impl<'a, K> DefaultSourceNestedCallablesV1<'a, K> {
    fn from_body(
        template: K,
        body: &'a ExportDefaultBodyV1,
        locals: &'a CanonicalTemplateLocalTableV1,
        origin: &'a ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut index = Self {
            template,
            occurrences: Vec::new(),
        };
        body.visit_direct_references(locals, origin, &mut index, meter, path)?;
        Ok(index)
    }
}
