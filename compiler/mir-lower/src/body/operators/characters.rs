//! Bound scalar references use the same operations as direct Char calls.

use super::*;

pub(super) fn character_member(
    kind: scoop_hir::CharIntrinsic,
    args: Vec<smir::Expr>,
    result_type: &mir::Type,
) -> smir::Expr {
    use scoop_hir::CharIntrinsic;
    let int = mir::IntegerKind::SIGNED_32;
    let code = |value| {
        smir::Expr::new(
            mir::Type::Integer(int),
            smir::ExprKind::CharCode(Box::new(value)),
        )
    };
    match kind {
        CharIntrinsic::Code => {
            let [value] = args.try_into().expect("Char.code has one receiver");
            code(value)
        }
        CharIntrinsic::FromCodeUnchecked => {
            let [value] = args
                .try_into()
                .expect("Char construction has one code point");
            smir::Expr::new(
                result_type.clone(),
                smir::ExprKind::CharFromCodeUnchecked(Box::new(value)),
            )
        }
        CharIntrinsic::Equals | CharIntrinsic::CompareTo => {
            let [lhs, rhs] = args.try_into().expect("Char comparison has two operands");
            let lhs = code(lhs);
            let rhs = code(rhs);
            if kind == CharIntrinsic::Equals {
                smir::Expr::integer_compare(
                    mir::IntegerComparisonOperation::new(
                        int,
                        mir::IntegerComparisonOperator::Equal,
                    ),
                    lhs,
                    rhs,
                )
            } else {
                smir::Expr::integer_compare_to(mir::IntegerCompareToOperation::new(int), lhs, rhs)
            }
        }
    }
}
