//! Borrowed nested declarations in the shared default body.
use crate::*;
use scoop_wire::{WireError, WirePath};

mod attachment;
mod descriptor;
mod query;
mod visit;
pub use descriptor::DefaultSourceNestedCallableDescriptorV1;
pub use query::DefaultSourceNestedCallableQueryError;

/// Nested callable descriptors borrowed from the complete default body.
#[derive(Debug)]
pub struct DefaultSourceNestedCallablesV1<'a> {
    occurrences: Vec<DefaultSourceNestedCallableOccurrenceV1<'a>>,
}
#[derive(Clone, Copy, Debug)]
pub struct DefaultSourceNestedCallableOccurrenceV1<'a> {
    site: DefaultNestedCallableSiteV1,
    descriptor: DefaultSourceNestedCallableDescriptorV1<'a>,
    definition_origin: &'a ExportDefinitionSourceV1,
}
impl<'a> DefaultSourceNestedCallablesV1<'a> {
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
        path: &WirePath,
    ) -> Result<DefaultSourceNestedCallablesV1<'_>, WireError> {
        DefaultSourceNestedCallablesV1::from_body(
            self.body(),
            self.locals(),
            self.definition_origin(),
            path,
        )
    }
}
impl<'a> DefaultSourceNestedCallablesV1<'a> {
    fn from_body(
        body: &'a ExportDefaultBodyV1,
        locals: &'a CanonicalTemplateLocalTableV1,
        origin: &'a ExportDefinitionSourceV1,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut index = Self {
            occurrences: Vec::new(),
        };
        body.visit_direct_references(locals, origin, &mut index, path)?;
        Ok(index)
    }
}
