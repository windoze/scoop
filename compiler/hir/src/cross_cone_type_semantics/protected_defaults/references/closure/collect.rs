use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::ProtectedDefaultBodyClosureError;

mod index;
use crate::{
    CanonicalTemplateLocalTableV1, DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceVisitorV1,
    DefaultExpressionV1, ExportDefaultBodyV1, ExportDefinitionSourceV1,
};
pub(crate) use index::DefaultReferenceExpressionIndexV1;

pub(super) struct Collected<'a> {
    pub expressions: DefaultReferenceExpressionIndexV1,
    pub occurrences: Vec<DefaultBodyReferenceOccurrenceV1<'a>>,
}
pub(super) fn collect<'a, E>(
    body: &'a ExportDefaultBodyV1,
    locals: &'a CanonicalTemplateLocalTableV1,
    origin: &'a ExportDefinitionSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Collected<'a>, ProtectedDefaultBodyClosureError<E>> {
    let mut collected = Collected {
        expressions: DefaultReferenceExpressionIndexV1::default(),
        occurrences: Vec::new(),
    };
    body.visit_direct_references(locals, origin, &mut collected, meter, path)?;
    Ok(collected)
}

impl<'body> DefaultBodyReferenceVisitorV1<'body> for Collected<'body> {
    type Error = WireError;
    fn expression(
        &mut self,
        index: u32,
        expression: &'body DefaultExpressionV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Self::Error> {
        self.expressions.insert(index, expression, meter, path)
    }
    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Self::Error> {
        meter.try_reserve_collection_slots(&mut self.occurrences, 1, path)?;
        self.occurrences.push(occurrence);
        Ok(())
    }
}
