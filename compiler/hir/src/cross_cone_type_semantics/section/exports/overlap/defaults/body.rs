use super::*;

pub(super) fn charge(
    body: &ExportDefaultBodyV1,
    locals: &CanonicalTemplateLocalTableV1,
    definition: &ExportDefinitionSourceV1,
    selector_cost: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    let before = meter.usage().decoded_nodes;
    body.visit_direct_references(locals, definition, &mut Leaves, meter, path)?;
    body.visit_definition_sources_metered(
        &mut |value, _, meter: &mut BudgetMeter, path: &WirePath| origin(value, meter, path),
        meter,
        path,
    )?;
    let nodes = meter
        .usage()
        .decoded_nodes
        .checked_sub(before)
        .ok_or_else(|| overflow(path))?;
    // Complete body validation bound every local use to this actual local table.
    // Four selectors plus closed scalar/id fields cover each visited constituent.
    let cost = selector_cost
        .checked_mul(4)
        .and_then(|n| n.checked_add(256))
        .and_then(|n| n.checked_mul(nodes))
        .ok_or_else(|| overflow(path))?;
    meter.charge_work(cost, path)
}

struct Leaves;
#[cfg(test)]
mod tests;
impl<'body> DefaultBodyReferenceVisitorV1<'body> for Leaves {
    type Error = WireError;
    fn expression(
        &mut self,
        _: u32,
        expression: &'body DefaultExpressionV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), WireError> {
        match expression.kind() {
            DefaultExpressionKindV1::StringLiteral { value, .. } => text(value, meter, path)?,
            DefaultExpressionKindV1::Lambda(value) => {
                structural_path(value.definition_path(), meter, path)?;
            }
            DefaultExpressionKindV1::AnonymousFunction(value) => {
                structural_path(value.definition_path(), meter, path)?;
            }
            DefaultExpressionKindV1::CallableReference(value) => {
                structural_path(value.definition_path(), meter, path)?;
            }
            _ => {}
        }
        Ok(())
    }
    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), WireError> {
        if let DefaultBodyReferenceTargetV1::Type(value) = occurrence.target {
            signature(value, value, meter, path)?;
        }
        match occurrence.attachment {
            DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::Pattern(pattern),
            ) => {
                if let DefaultPatternViewV1::Literal { value, .. } = pattern.view() {
                    constant(value, meter, path)?;
                }
            }
            DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::LocalFunction(value),
            ) => {
                structural_path(value.definition_path(), meter, path)?;
            }
            _ => {}
        }
        Ok(())
    }
}
