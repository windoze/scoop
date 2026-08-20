use super::*;

pub(crate) fn atomic_object_layout(
    context: &LoweringContext,
    kind: scoop_identity::AtomicValueKind,
) -> StorageResult<lir::AtomicObjectLayoutV1> {
    Ok(lir::atomic_object_layout(context.target_profile(), kind)?)
}
