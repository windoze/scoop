use super::*;
use std::collections::HashMap;

/// Addresses locate borrowed nodes in this traversal, never persistent entities.
#[derive(Default)]
pub(crate) struct DefaultReferenceExpressionIndexV1 {
    expressions: HashMap<*const DefaultExpressionV1, u32>,
}
impl DefaultReferenceExpressionIndexV1 {
    pub fn collect(
        body: &ExportDefaultBodyV1,
        locals: &CanonicalTemplateLocalTableV1,
        origin: &ExportDefinitionSourceV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut index = Self::default();
        body.visit_direct_references(locals, origin, &mut index, meter, path)?;
        Ok(index)
    }
    pub fn insert(
        &mut self,
        index: u32,
        expression: &DefaultExpressionV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), WireError> {
        meter.charge_work(1, path)?;
        meter.try_reserve_map_slots(&mut self.expressions, 1, path)?;
        self.expressions
            .insert(std::ptr::from_ref(expression), index);
        Ok(())
    }
    pub fn get(
        &self,
        expression: &DefaultExpressionV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<u32>, WireError> {
        meter.charge_work(1, path)?;
        Ok(self
            .expressions
            .get(&std::ptr::from_ref(expression))
            .copied())
    }
}
impl<'a> DefaultBodyReferenceVisitorV1<'a> for DefaultReferenceExpressionIndexV1 {
    type Error = WireError;
    fn expression(
        &mut self,
        index: u32,
        expression: &'a DefaultExpressionV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), WireError> {
        self.insert(index, expression, meter, path)
    }
    fn reference(
        &mut self,
        _: DefaultBodyReferenceOccurrenceV1<'a>,
        _: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<(), WireError> {
        Ok(())
    }
}
