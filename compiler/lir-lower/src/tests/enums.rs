use super::*;

/// The LIR enum definition transposed from a MIR enum (the arenas
/// align 1:1).
fn edef(module: &lir::Module, id: mir::EnumId) -> &lir::EnumDef {
    &module.enums[lir::EnumDefId::from_raw(id.into_raw())]
}

/// main holding `o: Option<T>` through a None / tag / field /
/// wrap round-trip; shared shell of the two representation tests.
fn option_round_trip(name: &str, payload: mir::Type) -> mir::Module {
    let mut b = Builder::new();
    let option = b.option_enum(name, payload.clone());
    let option_ty = mir::Type::Enum(option, vec![payload.clone()]);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_ty.clone()));
    let t = locals.alloc(local("t", mir::Type::Int));
    let p = locals.alloc(local("p", payload.clone()));
    let o2 = locals.alloc(local("o2", option_ty.clone()));
    let main = b.main(
        locals,
        vec![
            // None
            val_decl(
                o,
                expr(
                    option_ty.clone(),
                    mir::ExprKind::VariantConstruct {
                        variant: 1,
                        fields: Vec::new(),
                    },
                ),
            ),
            val_decl(
                t,
                expr(
                    mir::Type::Int,
                    mir::ExprKind::EnumTag(Box::new(local_expr(o, option_ty.clone()))),
                ),
            ),
            val_decl(
                p,
                expr(
                    payload.clone(),
                    mir::ExprKind::EnumField {
                        operand: Box::new(local_expr(o, option_ty.clone())),
                        variant: 0,
                        index: 0,
                    },
                ),
            ),
            val_decl(
                o2,
                expr(
                    option_ty,
                    mir::ExprKind::VariantConstruct {
                        variant: 0,
                        fields: vec![local_expr(p, payload)],
                    },
                ),
            ),
        ],
    );
    b.finish(main)
}

mod niche;
mod tagged;
