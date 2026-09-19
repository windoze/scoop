use super::*;

mod layout;
mod records;
mod resolve;

pub(super) fn enqueue(
    record: LayoutAbiSemanticRecordV1<'_>,
    owner: usize,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let next = depth
        .checked_add(1)
        .ok_or(LayoutAbiSemanticClosureError::ArithmeticOverflow)?;
    match record {
        LayoutAbiSemanticRecordV1::Layout(record) => {
            layout::enqueue(record, next, views, index, pending, meter)
        }
        LayoutAbiSemanticRecordV1::Descriptor(record) => {
            records::descriptor(record, owner, next, views, index, pending, meter)
        }
        LayoutAbiSemanticRecordV1::Dispatch(record) => {
            records::dispatch(record, owner, next, views, index, pending, meter)
        }
        LayoutAbiSemanticRecordV1::Callable(record) => {
            records::callable(record, next, views, index, pending, meter)
        }
        LayoutAbiSemanticRecordV1::ShapeSupport(record) => {
            records::shape_support(record, next, views, index, pending, meter)
        }
    }
}
