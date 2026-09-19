use std::collections::HashMap;

use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::ProtectedDefaultBodyClosureError;
use crate::{
    CanonicalTemplateLocalTableV1, DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceVisitorV1,
    DefaultExpressionV1, ExportDefaultBodyV1, ExportDefinitionSourceV1,
};

pub(super) struct Collected<'a> {
    // Addresses identify borrowed tree nodes within this traversal only. They
    // are never serialized or used as source/entity identities.
    expressions: HashMap<*const DefaultExpressionV1, u32>,
    pub occurrences: Vec<DefaultBodyReferenceOccurrenceV1<'a>>,
}
impl Collected<'_> {
    pub fn expression_index<E>(
        &self,
        expression: &DefaultExpressionV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<u32, ProtectedDefaultBodyClosureError<E>> {
        meter.charge_work(1, path)?;
        self.expressions
            .get(&std::ptr::from_ref(expression))
            .copied()
            .ok_or(ProtectedDefaultBodyClosureError::ReceiverOutsideBody)
    }
}

pub(super) fn collect<'a, E>(
    body: &'a ExportDefaultBodyV1,
    locals: &'a CanonicalTemplateLocalTableV1,
    origin: &'a ExportDefinitionSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Collected<'a>, ProtectedDefaultBodyClosureError<E>> {
    let mut collected = Collected {
        expressions: HashMap::new(),
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
        meter.try_reserve_map_slots(&mut self.expressions, 1, path)?;
        self.expressions
            .insert(std::ptr::from_ref(expression), index);
        Ok(())
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
