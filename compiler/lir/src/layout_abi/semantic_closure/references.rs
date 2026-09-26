use super::*;

mod layout;
mod records;
mod resolve;

pub(super) fn enqueue(
    record: LayoutAbiSemanticRecordV1<'_>,
    owner: usize,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    match record {
        LayoutAbiSemanticRecordV1::Layout(record) => layout::enqueue(record, views, index, pending),
        LayoutAbiSemanticRecordV1::Descriptor(record) => {
            records::descriptor(record, owner, views, index, pending)
        }
        LayoutAbiSemanticRecordV1::Dispatch(record) => {
            records::dispatch(record, owner, views, index, pending)
        }
        // Signatures contain logical types and ABI storage, not addressable
        // layout references. Actual machine uses carry their own relocations.
        LayoutAbiSemanticRecordV1::Callable(_) | LayoutAbiSemanticRecordV1::DirectCallable(_) => {
            Ok(())
        }
        LayoutAbiSemanticRecordV1::ShapeSupport(record) => {
            records::shape_support(record, views, index, pending)
        }
    }
}
