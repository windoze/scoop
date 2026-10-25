use super::*;

fn known(expression: &mir::Expr) -> Option<Constant> {
    evaluate(expression, known)
}

#[test]
fn character_constants_preserve_scalar_boundaries_and_unchecked_preconditions() {
    let owner = mir::StructId::from_raw(la_arena::RawIdx::from_u32(7));
    for scalar in [
        0, 127, 128, 2047, 2048, 0xd7ff, 0xe000, 0xffff, 0x10000, 0x10ffff,
    ] {
        let character = char::from_u32(scalar).unwrap();
        let value = Constant::Character {
            owner,
            value: character,
        };
        assert_eq!(known(&value.expression()), Some(value));
        let code = mir::Expr::new(
            mir::Type::Integer(mir::IntegerKind::SIGNED_32),
            mir::ExprKind::CharCode(Box::new(value.expression())),
        );
        assert_eq!(
            known(&code),
            Some(Constant::Integer(mir::MirIntegerConstant::Signed32(scalar)))
        );
        let rebuilt = mir::Expr::new(
            mir::Type::Struct(owner),
            mir::ExprKind::CharFromCodeUnchecked(Box::new(code)),
        );
        assert_eq!(known(&rebuilt), Some(value));
    }
    for scalar in [u32::MAX, 0xd800, 0xdfff, 0x110000] {
        let expression = mir::Expr::new(
            mir::Type::Struct(owner),
            mir::ExprKind::CharFromCodeUnchecked(Box::new(mir::Expr::integer(
                mir::MirIntegerConstant::Signed32(scalar),
            ))),
        );
        assert_eq!(known(&expression), None);
    }
}
