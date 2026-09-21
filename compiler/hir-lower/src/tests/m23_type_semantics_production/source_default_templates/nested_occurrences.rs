use super::*;

#[test]
fn expanded_source_references_preserve_distinct_abis_for_one_invoke_identity() {
    with_hir_source(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/nested-occurrences.scoop"
        )),
        |output, _| {
            let original = template(output, "OccurrenceHost.pair", 0);
            let restored = round_trip(output, &original);
            let mut collector = References(Vec::new());
            restored
                .body()
                .visit_direct_references(
                    restored.locals(),
                    restored.definition_origin(),
                    &mut collector,
                    &mut meter(),
                    &scoop_wire::WirePath::root(),
                )
                .unwrap();
            let [left, right] = collector.0.as_slice() else {
                panic!("two expanded reference descriptors are required");
            };
            assert_eq!(left.invoke(), right.invoke());
            assert_eq!(left.definition_path(), right.definition_path());
            assert_eq!(left.owner_type_parameter_count(), 1);
            assert_eq!(right.owner_type_parameter_count(), 1);
            assert_ne!(left.function_type(), right.function_type());
        },
    );
}

struct References<'a>(Vec<&'a hir::DefaultCallableReferenceV1>);
impl<'a> hir::DefaultBodyReferenceVisitorV1<'a> for References<'a> {
    type Error = scoop_wire::WireError;
    fn expression(
        &mut self,
        _: u32,
        expression: &'a hir::DefaultExpressionV1,
        meter: &mut BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<(), Self::Error> {
        if let hir::DefaultExpressionKindV1::CallableReference(reference) = expression.kind() {
            meter.try_reserve_collection_slots(&mut self.0, 1, path)?;
            self.0.push(reference);
        }
        Ok(())
    }
    fn reference(
        &mut self,
        _: hir::DefaultBodyReferenceOccurrenceV1<'a>,
        _: &mut BudgetMeter,
        _: &scoop_wire::WirePath,
    ) -> Result<(), Self::Error> {
        // Only expression descriptors are relevant to this ABI comparison.
        Ok(())
    }
}
