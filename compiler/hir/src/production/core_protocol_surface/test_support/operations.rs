use super::*;

pub(super) fn operation_entry(
    operations: &[(IntrinsicFunctionKind, CoreProtocolCallableV1)],
    kind: IntrinsicFunctionKind,
) -> CoreProtocolCallableV1 {
    operations
        .iter()
        .find(|operation| operation.0 == kind)
        .expect("the test fixture constructs every intrinsic operation")
        .1
        .clone()
}
