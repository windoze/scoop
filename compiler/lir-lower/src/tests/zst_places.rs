use super::*;

fn address_of(local: mir::LocalId, ty: mir::Type) -> mir::Statement {
    expr_stmt(expr(
        mir::Type::Ptr(Box::new(ty.clone())),
        mir::ExprKind::AddressOf {
            local,
            pointee: Box::new(ty),
        },
    ))
}

#[test]
fn zst_local_parameter_and_value_this_have_explicit_independent_place_plans() {
    let mut builder = Builder::new();
    let first = builder.strukt("First", &[]);
    let second = builder.strukt("Second", &[]);
    let first_ty = mir::Type::Struct(first);
    let second_ty = mir::Type::Struct(second);
    let mut locals = Arena::new();
    let parameter = locals.alloc(local("parameter", first_ty.clone()));
    let receiver = locals.alloc(local("this", first_ty.clone()));
    let untouched = locals.alloc(local("untouched", first_ty.clone()));
    let addressed = locals.alloc(local("addressed", first_ty.clone()));
    let logical = locals.alloc(local("logical", second_ty.clone()));
    let function = builder.user_fn_full(
        "observePlaces",
        [
            ("parameter", parameter),
            ("this", receiver),
            ("untouched", untouched),
        ]
        .into_iter()
        .map(|(name, local)| mir::Param {
            name: name.to_string(),
            ty: first_ty.clone(),
            local,
        })
        .collect(),
        mir::Type::Unit,
        locals,
        vec![
            val_decl(addressed, local_expr(parameter, first_ty.clone())),
            val_decl(
                logical,
                expr(
                    second_ty.clone(),
                    mir::ExprKind::StructConstruct {
                        struct_id: second,
                        fields: Vec::new(),
                    },
                ),
            ),
            address_of(parameter, first_ty.clone()),
            address_of(receiver, first_ty.clone()),
            address_of(addressed, first_ty.clone()),
            address_of(addressed, first_ty.clone()),
        ],
    );
    let main = builder.main(Arena::new(), Vec::new());
    let source = builder.finish(main);
    let first_exact = exact_type_record(&source, &first_ty).id();
    let second_exact = exact_type_record(&source, &second_ty).id();
    assert_ne!(first_exact, second_exact);
    let lowered = lower(source);
    let function = &lowered.functions[function.into_raw().into_u32() as usize];
    assert_eq!(function.signature.logical_argument_count(), 3);
    assert_eq!(function.signature.physical_parameter_count(), 0);
    let places: Vec<_> = function.locals.iter().collect();
    assert_eq!(
        places.len(),
        4,
        "unobserved parameter must remain an SSA value"
    );
    for (_, place) in &places[..3] {
        let lir::LocalStorage::AddressableZst(token) = place.storage() else {
            panic!("{} requires a distinct address token", place.name);
        };
        assert_eq!(token.value().exact(), first_exact);
        assert_eq!(token.value().representation().layout().alignment().get(), 1);
        assert_eq!(
            token.lifetime(),
            lir::LocalPlaceLifetime::FunctionActivation
        );
    }
    let lir::LocalStorage::LogicalZst(value) = places[3].1.storage() else {
        panic!("an unobserved ZST local must not acquire a token");
    };
    assert_eq!(value.exact(), second_exact);
    let addresses: Vec<_> = function.blocks[function.entry]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            lir::Instruction::LocalAddress { local, .. } => Some(*local),
            _ => None,
        })
        .collect();
    assert_eq!(
        addresses,
        vec![places[0].0, places[1].0, places[2].0, places[2].0]
    );
    let dump = lir::dump(&lowered);
    let locals: Vec<_> = dump
        .lines()
        .filter(|line| line.contains("    local "))
        .collect();
    insta::assert_snapshot!(locals.join("\n"));
}

#[test]
fn mixed_zst_signature_preserves_logical_indexes_around_sret() {
    let mut builder = Builder::new();
    let empty = builder.strukt("Empty", &[]);
    let triple = builder.strukt(
        "Triple",
        &[("first", LONG), ("second", LONG), ("third", LONG)],
    );
    let types = [
        mir::Type::Struct(empty),
        INT,
        mir::Type::Struct(empty),
        mir::Type::Struct(triple),
    ];
    let mut locals = Arena::new();
    let params = types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let name = format!("p{index}");
            let local = locals.alloc(local(&name, ty.clone()));
            mir::Param {
                name,
                ty: ty.clone(),
                local,
            }
        })
        .collect();
    let function = builder.user_fn_full(
        "mixed",
        params,
        mir::Type::Struct(triple),
        locals,
        Vec::new(),
    );
    let main = builder.main(Arena::new(), Vec::new());
    let lowered = lower(builder.finish(main));
    let signature = &lowered.functions[function.into_raw().into_u32() as usize].signature;
    assert!(matches!(signature.result(), lir::AbiReturn::Indirect(_)));
    assert_eq!(signature.logical_argument_count(), 4);
    assert_eq!(signature.physical_parameter_count(), 3);
    assert_eq!(
        signature.argument_location(0),
        Some(lir::AbiArgumentLocation::Elided)
    );
    assert_eq!(
        signature.argument_location(1),
        Some(lir::AbiArgumentLocation::Parameter(1))
    );
    assert_eq!(
        signature.argument_location(2),
        Some(lir::AbiArgumentLocation::Elided)
    );
    assert_eq!(
        signature.argument_location(3),
        Some(lir::AbiArgumentLocation::Parameter(2))
    );
}
