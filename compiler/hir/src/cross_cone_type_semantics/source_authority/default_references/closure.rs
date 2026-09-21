//! Exact, ordered correspondence between source records and typed body occurrences.
use crate::*;
use scoop_wire::{BudgetMeter, WireError, WirePath};
mod errors;
mod records;
mod visitor;
pub use errors::*;
pub use records::DefaultSourceReferenceRecordV1;

/// Coverage and occurrence correspondence only; no access or execution capability.
#[derive(Debug)]
pub struct DefaultSourceReferenceClosureV1<'a> {
    template: ProtectedDefaultTemplateKeyV1,
    occurrences: Vec<DefaultSourceReferenceOccurrenceV1<'a>>,
}
#[derive(Clone, Copy, Debug)]
pub struct DefaultSourceReferenceOccurrenceV1<'a> {
    index: u32,
    body: DefaultBodyReferenceOccurrenceV1<'a>,
    source: DefaultSourceReferenceRecordV1<'a>,
    context: DefaultReferenceContextV1<'a>,
}
impl DefaultSourceTemplateV1 {
    pub fn bind_reference_occurrences(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultSourceReferenceClosureV1<'_>, DefaultSourceReferenceClosureError> {
        let expressions = DefaultReferenceExpressionIndexV1::collect(
            self.body(),
            self.locals(),
            self.definition_origin(),
            meter,
            path,
        )?;
        let mut visitor = visitor::Visitor::new(self, &expressions, meter, path)?;
        self.body().visit_direct_references(
            self.locals(),
            self.definition_origin(),
            &mut visitor,
            meter,
            path,
        )?;
        visitor.finish(meter, path)
    }
}
impl<'a> DefaultSourceReferenceClosureV1<'a> {
    pub const fn template(&self) -> ProtectedDefaultTemplateKeyV1 {
        self.template
    }
    pub fn occurrences(&self) -> &[DefaultSourceReferenceOccurrenceV1<'a>] {
        &self.occurrences
    }
}
impl<'a> DefaultSourceReferenceOccurrenceV1<'a> {
    /// Actual body context; target and protected receiver permission remain separate.
    pub const fn context(&self) -> DefaultReferenceContextV1<'a> {
        self.context
    }
    pub const fn index(&self) -> u32 {
        self.index
    }
    pub const fn body(&self) -> DefaultBodyReferenceOccurrenceV1<'a> {
        self.body
    }
    pub const fn source(&self) -> DefaultSourceReferenceRecordV1<'a> {
        self.source
    }
}
