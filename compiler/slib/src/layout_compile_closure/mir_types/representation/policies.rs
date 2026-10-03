use scoop_hir as hir;
use scoop_mir as mir;

pub(in crate::layout_compile_closure::mir_types) fn c_layout(
    source: hir::NominalCLayoutPolicyV1,
) -> mir::MirTypeCLayoutPolicyV1 {
    match source {
        hir::NominalCLayoutPolicyV1::Ordinary => mir::MirTypeCLayoutPolicyV1::Ordinary,
        hir::NominalCLayoutPolicyV1::CLayout { contract } => {
            mir::MirTypeCLayoutPolicyV1::CLayout(mir::MirCLayoutContract {
                aligned: alignment(contract.aligned),
                packed: alignment(contract.packed),
            })
        }
    }
}

fn alignment(source: hir::HirCLayoutValue) -> mir::MirCLayoutValue {
    match source {
        hir::HirCLayoutValue::Natural => mir::MirCLayoutValue::Natural,
        hir::HirCLayoutValue::A1 => mir::MirCLayoutValue::A1,
        hir::HirCLayoutValue::A2 => mir::MirCLayoutValue::A2,
        hir::HirCLayoutValue::A4 => mir::MirCLayoutValue::A4,
        hir::HirCLayoutValue::A8 => mir::MirCLayoutValue::A8,
        hir::HirCLayoutValue::A16 => mir::MirCLayoutValue::A16,
    }
}

pub(super) fn intrinsic(source: hir::IntrinsicTypeKind) -> Option<mir::MirParamFreeIntrinsicV1> {
    Some(match source {
        hir::IntrinsicTypeKind::Integer(kind) => {
            mir::MirParamFreeIntrinsicV1::Integer(mir::IntegerKind::new(
                match kind.signedness() {
                    hir::IntegerSignedness::Signed => mir::IntegerSignedness::Signed,
                    hir::IntegerSignedness::Unsigned => mir::IntegerSignedness::Unsigned,
                },
                match kind.width() {
                    hir::IntegerWidth::W8 => mir::IntegerWidth::W8,
                    hir::IntegerWidth::W16 => mir::IntegerWidth::W16,
                    hir::IntegerWidth::W32 => mir::IntegerWidth::W32,
                    hir::IntegerWidth::W64 => mir::IntegerWidth::W64,
                },
            ))
        }
        hir::IntrinsicTypeKind::Boolean => mir::MirParamFreeIntrinsicV1::Boolean,
        hir::IntrinsicTypeKind::String => mir::MirParamFreeIntrinsicV1::String,
        hir::IntrinsicTypeKind::Array
        | hir::IntrinsicTypeKind::MutableArray
        | hir::IntrinsicTypeKind::Ptr
        | hir::IntrinsicTypeKind::FunPtr => return None,
    })
}
