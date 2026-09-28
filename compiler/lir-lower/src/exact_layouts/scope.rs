//! Export demand is independent of unrelated local physical definitions.

use super::*;

impl Projection<'_> {
    pub(super) fn collect_export_roots(&mut self) -> Result<()> {
        for root in self.output.shape_support().roots() {
            for shape in [root.source(), root.coroutine_step(), root.coroutine_slot()]
                .into_iter()
                .chain(match root.boxed_value() {
                    lir::StrongLirBoxedValueMaterialization::Available(shape) => Some(shape),
                    lir::StrongLirBoxedValueMaterialization::NotApplicable => None,
                })
            {
                if !self.exported(shape.exact()) {
                    return Err(ExactLayoutLoweringError::MissingMirShape(shape.exact()));
                }
            }
        }
        let inline_payloads: BTreeSet<_> = self
            .types
            .records()
            .iter()
            .filter_map(|record| match record.representation() {
                mir::MirTypeRepresentationV1::BoxedValue { payload } => Some(payload.value),
                mir::MirTypeRepresentationV1::InlineArray { element } => Some(*element),
                _ => None,
            })
            .collect();
        for (_, layout) in self.output.module().meta.layouts.iter() {
            let record = layout.identity.layout_record();
            if self.exported(record.key().exact_type())
                || (record.key().representation() == RepresentationRole::ManagedValue
                    && inline_payloads.contains(&record.key().exact_type()))
            {
                self.add_root(record)?;
            }
        }
        for (_, descriptor) in self.output.module().meta.type_descriptors.iter() {
            if self.exported(descriptor.identity.exact_type()) {
                self.add_root(descriptor.instance_layout.layout_record())?;
            }
        }
        Ok(())
    }

    fn exported(&mut self, exact: PersistentExactTypeId) -> bool {
        self.types.get(exact).is_some()
    }
}
