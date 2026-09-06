use super::*;

fn lower_variant_primitives(
    enum_name: &str,
    payload_ty: mir::Type,
    payload: mir::Expr,
) -> lir::Module {
    let mut builder = Builder::new();
    let option = builder.option_enum(enum_name, payload_ty.clone());
    let option_ty = mir::Type::Enum(option, vec![payload_ty.clone()]);
    let option_core = builder.option_core[0];
    let some = option_core.some();
    let some_payload = option_core.some_payload();

    let mut locals = Arena::new();
    let subject = locals.alloc(local("subject", option_ty.clone()));
    let projected = locals.alloc(local("projected", payload_ty.clone()));

    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let matched = cfg_block(&mut blocks, "variant.matched");
    let missed = cfg_block(&mut blocks, "variant.missed");
    let exit = cfg_block(&mut blocks, "exit");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![val_decl(
            subject,
            expr(
                option_ty.clone(),
                mir::ExprKind::VariantConstruct {
                    variant: some,
                    fields: vec![payload],
                },
            ),
        )],
        mir::Terminator::Branch {
            cond: mir::Expr::variant_test(
                &builder.enums,
                local_expr(subject, option_ty.clone()),
                some,
            )
            .expect("typed variant test"),
            then_block: matched,
            else_block: missed,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        matched,
        vec![val_decl(
            projected,
            mir::Expr::variant_payload_project(
                &builder.enums,
                local_expr(subject, option_ty),
                some_payload,
            )
            .expect("typed payload projection"),
        )],
        mir::Terminator::Goto(exit),
        None,
    );
    set_cfg_block(
        &mut blocks,
        missed,
        Vec::new(),
        mir::Terminator::Goto(exit),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = builder.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    lower(&builder.finish(main))
}

fn primitive_instructions(
    module: &lir::Module,
) -> (&lir::Instruction, &lir::Instruction, &lir::Instruction) {
    let function = &module.functions[0];
    let wrap = function
        .blocks
        .iter()
        .find_map(|(_, block)| {
            block
                .instructions
                .iter()
                .find(|instruction| matches!(instruction, lir::Instruction::EnumWrap { .. }))
        })
        .expect("lowered typed variant construction");
    let test = function
        .blocks
        .iter()
        .find_map(|(_, block)| {
            block
                .instructions
                .iter()
                .find(|instruction| matches!(instruction, lir::Instruction::VariantTest { .. }))
        })
        .expect("lowered variant test");
    let project = function
        .blocks
        .iter()
        .find_map(|(_, block)| {
            block.instructions.iter().find(|instruction| {
                matches!(instruction, lir::Instruction::VariantPayloadProject { .. })
            })
        })
        .expect("lowered payload projection");
    (wrap, test, project)
}

#[test]
fn typed_variant_primitives_lower_through_tagged_layout() {
    let module = lower_variant_primitives("Option$I", INT, int_expr(7));
    let (wrap, test, project) = primitive_instructions(&module);
    let lir::Instruction::EnumWrap {
        variant: wrapped, ..
    } = wrap
    else {
        unreachable!()
    };
    let lir::Instruction::VariantTest { variant, .. } = test else {
        unreachable!()
    };
    let lir::Instruction::VariantPayloadProject { field, out, .. } = project else {
        unreachable!()
    };

    assert_eq!(wrapped, variant);
    assert_eq!(variant.index(), 0);
    assert_eq!(field.variant(), *variant);
    assert_eq!(field.index(), 0);
    assert_eq!(module.functions[0].temps[*out].ty, lir::LirType::I32);
    assert!(matches!(
        module.enums[variant.definition()].repr,
        lir::EnumRepr::Tagged { .. }
    ));

    let dump = lir::dump(&module);
    assert!(dump.contains("variant_test e0 v0 local0 : i1"), "{dump}");
    assert!(
        dump.contains("variant_payload_project e0 v0 f0 local0 : i32"),
        "{dump}"
    );
}

#[test]
fn typed_variant_primitives_lower_through_niche_layout() {
    let mut builder = Builder::new();
    let payload = builder.string("payload");
    let option = builder.option_enum("Option$S", mir::Type::String);
    let option_ty = mir::Type::Enum(option, vec![mir::Type::String]);
    let option_core = builder.option_core[0];
    let some = option_core.some();
    let some_payload = option_core.some_payload();

    let mut locals = Arena::new();
    let subject = locals.alloc(local("subject", option_ty.clone()));
    let projected = locals.alloc(local("projected", mir::Type::String));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let matched = cfg_block(&mut blocks, "variant.matched");
    let missed = cfg_block(&mut blocks, "variant.missed");
    let exit = cfg_block(&mut blocks, "exit");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![val_decl(
            subject,
            expr(
                option_ty.clone(),
                mir::ExprKind::VariantConstruct {
                    variant: some,
                    fields: vec![string_expr(payload)],
                },
            ),
        )],
        mir::Terminator::Branch {
            cond: mir::Expr::variant_test(
                &builder.enums,
                local_expr(subject, option_ty.clone()),
                some,
            )
            .expect("typed variant test"),
            then_block: matched,
            else_block: missed,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        matched,
        vec![val_decl(
            projected,
            mir::Expr::variant_payload_project(
                &builder.enums,
                local_expr(subject, option_ty),
                some_payload,
            )
            .expect("typed payload projection"),
        )],
        mir::Terminator::Goto(exit),
        None,
    );
    set_cfg_block(
        &mut blocks,
        missed,
        Vec::new(),
        mir::Terminator::Goto(exit),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = builder.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&builder.finish(main));
    let (wrap, test, project) = primitive_instructions(&module);
    let lir::Instruction::EnumWrap {
        variant: wrapped, ..
    } = wrap
    else {
        unreachable!()
    };
    let lir::Instruction::VariantTest { variant, .. } = test else {
        unreachable!()
    };
    let lir::Instruction::VariantPayloadProject { field, out, .. } = project else {
        unreachable!()
    };

    assert_eq!(wrapped, variant);
    assert_eq!(field.variant(), *variant);
    assert_eq!(module.functions[0].temps[*out].ty, lir::MANAGED_PTR);
    assert!(matches!(
        module.enums[variant.definition()].repr,
        lir::EnumRepr::Niche {
            kind: lir::NichePointerKind::Managed,
            payload_variant: 0,
        }
    ));
}
