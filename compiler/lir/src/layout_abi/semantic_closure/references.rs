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
        LayoutAbiSemanticRecordV1::Callable(record) => {
            records::callable(record, views, index, pending)
        }
        LayoutAbiSemanticRecordV1::ShapeSupport(record) => {
            records::shape_support(record, views, index, pending)
        }
    }
}
