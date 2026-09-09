use super::super::*;

#[test]
fn enum_instances_are_created_once_with_substituted_fields() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    // A non-generic enum.
    let color_variants = ["Red", "Green", "Blue"]
        .iter()
        .map(|name| hir::Variant {
            name: name.to_string(),
            fields: Vec::new(),
        })
        .collect();
    let color = h.declare_enum("Color", Vec::new(), Vec::new(), color_variants);
    let color_ty = h.enum_ty(color);
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let option_int = h.option(int);
    let option_string = h.option(string);
    let option_s = h.option(s_ty);
    // f1 holds Option<Int> and Color; f2 holds Option<Int> again
    // (a duplicate request) and Option<String>.
    let mut locals1 = Arena::new();
    locals1.alloc(local("o", option_int));
    locals1.alloc(local("c", color_ty));
    let _f1 = h.user_fn(
        "f1",
        hir::Body {
            locals: locals1,
            statements: Vec::new(),
        },
    );
    let mut locals2 = Arena::new();
    locals2.alloc(local("o", option_int));
    locals2.alloc(local("s", option_string));
    let _f2 = h.user_fn(
        "f2",
        hir::Body {
            locals: locals2,
            statements: Vec::new(),
        },
    );
    let mut locals3 = Arena::new();
    locals3.alloc(local("s", option_s));
    let _f3 = h.user_fn(
        "f3",
        hir::Body {
            locals: locals3,
            statements: Vec::new(),
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    // One definition per (enum, type args), in creation order; the
    // duplicate Option<Int> request was deduplicated by enum identity.
    let names: Vec<&str> = module
        .enums
        .iter()
        .map(|(_, def)| def.name.as_str())
        .collect();
    assert_eq!(names, ["Option$I32", "Color", "Option$S", "Option$D1_SX"]);

    // The variant field types are substituted with the instance's
    // type arguments.
    let option_int_def = &module.enums[la_arena::Idx::from_raw(0.into())];
    assert_eq!(option_int_def.variants[0].name, "Some");
    assert_eq!(
        option_int_def.variants[0].fields[0].ty,
        mir::Type::Integer(mir::IntegerKind::SIGNED_32)
    );
    assert!(option_int_def.gc_free);
    assert!(
        option_int_def
            .variants
            .iter()
            .all(|variant| variant.gc_free)
    );
    let option_string_def = &module.enums[la_arena::Idx::from_raw(2.into())];
    assert_eq!(
        option_string_def.variants[0].fields[0].ty,
        mir::Type::String
    );
    let option_s_def = &module.enums[la_arena::Idx::from_raw(3.into())];
    assert_eq!(
        option_s_def.variants[0].fields[0].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert_ne!(option_string_def.name, option_s_def.name);
    assert!(!option_string_def.gc_free);
    assert!(!option_string_def.variants[0].gc_free);
    assert!(option_string_def.variants[1].gc_free);
    assert_eq!(module.option_core.len(), 3);
    for enum_id in [
        la_arena::Idx::from_raw(0.into()),
        la_arena::Idx::from_raw(2.into()),
        la_arena::Idx::from_raw(3.into()),
    ] {
        let option = module
            .option_core(enum_id)
            .expect("each concrete core Option has exact MIR provenance");
        assert_eq!(option.enum_id(), enum_id);
        assert_eq!(option.some_payload().variant(), option.some());
        assert_eq!(option.some_payload().field_index(), 0);
        assert_eq!(
            option.some().definition(&module.enums).unwrap().name,
            "Some"
        );
        assert_eq!(
            option
                .some()
                .definition(&module.enums)
                .unwrap()
                .fields
                .len(),
            1
        );
        assert_eq!(
            option.none().definition(&module.enums).unwrap().name,
            "None"
        );
        assert!(
            option
                .none()
                .definition(&module.enums)
                .unwrap()
                .fields
                .is_empty()
        );
        assert_ne!(option.some(), option.none());
    }
    assert!(
        module
            .option_core(la_arena::Idx::from_raw(1.into()))
            .is_none(),
        "an equal-shaped user enum must not be recognized as core Option"
    );
    // Color's variants are all unit variants.
    let color_def = &module.enums[la_arena::Idx::from_raw(1.into())];
    assert!(color_def.gc_free);
    assert_eq!(color_def.variants.len(), 3);
    assert!(
        color_def
            .variants
            .iter()
            .all(|variant| variant.fields.is_empty() && variant.gc_free)
    );
}

#[test]
fn option_consumers_become_guarded_representation_independent_primitives() {
    let mut h = Harness::new();
    let (int, boolean) = (h.int, h.boolean);
    let option_int = h.option(int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let n = locals.alloc(local("n", option_int));
    let y = locals.alloc(local("y", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 41))),
                        option_int,
                    ),
                ),
                val_decl(n, expr(hir::ExprKind::NoneLiteral, option_int)),
                stmt(hir::StatementKind::If {
                    cond: expr(
                        hir::ExprKind::IsSome(Box::new(local_ref(o, option_int))),
                        boolean,
                    ),
                    then_body: vec![val_decl(
                        y,
                        expr(
                            hir::ExprKind::Unwrap {
                                operand: Box::new(local_ref(o, option_int)),
                                trap_on_none: false,
                            },
                            int,
                        ),
                    )],
                    else_body: None,
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let expected = "\
Module
  enum Option$I32
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I32<Int>
        Type Option$I32<Int>
        VariantConstruct Option$I32<Int> v0
          Type Int
          IntegerLiteral Int value=41 bits=0x00000029
      val n: Option$I32<Int>
        Type Option$I32<Int>
        VariantConstruct Option$I32<Int> v1
      branch bb1 bb2
        Type Boolean
        VariantTest Option$I32 v0
          Type Option$I32<Int>
          Local o
    bb1 if.then.1
      val y: Int
        Type Int
        VariantPayloadProject Option$I32 v0 f0
          Type Option$I32<Int>
          Local o
      goto bb2
    bb2 if.merge.2
      return
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
    assert_eq!(module.validate(), Ok(()));
}

#[test]
fn trapping_unwrap_becomes_a_guarded_extraction() {
    // val o = Some(1); val y = o!!
    let mut h = Harness::new();
    h.exception("UnwrapException");
    let int = h.int;
    let option_int = h.option(int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let y = locals.alloc(local("y", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                        option_int,
                    ),
                ),
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(local_ref(o, option_int)),
                            trap_on_none: true,
                        },
                        int,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The operand is evaluated once into `$opt.1`; the semantic variant test
    // guards the representation-independent extraction, and the else branch throws
    // `UnwrapException()` (M8) — an ordinary constructor call.
    let expected = "\
Module
  enum Option$I32
    Some(_1: Int)
    None()
  class UnwrapException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I32<Int>
        Type Option$I32<Int>
        VariantConstruct Option$I32<Int> v0
          Type Int
          IntegerLiteral Int value=1 bits=0x00000001
      val $opt.1: Option$I32<Int>
        Type Option$I32<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        VariantTest Option$I32 v0
          Type Option$I32<Int>
          Local $opt.1
    bb1 if.then.1
      val $uw.2: Int
        Type Int
        VariantPayloadProject Option$I32 v0 f0
          Type Option$I32<Int>
          Local $opt.1
      goto bb3
    bb2 if.else.2
      assign $new.1
        Type UnwrapException
        ClassAlloc UnwrapException
      call @scoop.init.C15_UnwrapExceptionX.$c0 direct
        Type UnwrapException
        Local $new.1
      throw
        Type UnwrapException
        Local $new.1
    bb3 if.merge.3
      val y: Int
        Type Int
        Local $uw.2
      return
  fun init.UnwrapException.$c0 @scoop.init.C15_UnwrapExceptionX.$c0(this: UnwrapException) -> Unit
    bb0 entry
      return
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
    assert_eq!(module.validate(), Ok(()));
}

#[test]
fn option_primitives_cover_tagged_and_managed_raw_and_code_niche_payloads() {
    let mut h = Harness::new();
    let int = h.int;
    let string = h.string;
    let pointer = h.types.alloc(hir::Type::Ptr(int));
    let function_type = hir::FunctionTypeId::from_raw(0.into());
    let managed_function = h.types.alloc(hir::Type::Function(function_type));
    let function_pointer = h.types.alloc(hir::Type::FunPtr(function_type));
    let payload_types = [int, string, pointer, function_pointer];
    let option_types = payload_types.map(|payload| h.option(payload));

    let mut locals = Arena::new();
    let mut statements = Vec::new();
    for (payload_ty, option_ty) in payload_types.into_iter().zip(option_types) {
        let option = locals.alloc(local("option", option_ty));
        let payload = locals.alloc(local("payload", payload_ty));
        statements.push(val_decl(
            option,
            expr(hir::ExprKind::NoneLiteral, option_ty),
        ));
        statements.push(stmt(hir::StatementKind::If {
            cond: expr(
                hir::ExprKind::IsSome(Box::new(local_ref(option, option_ty))),
                h.boolean,
            ),
            then_body: vec![val_decl(
                payload,
                expr(
                    hir::ExprKind::Unwrap {
                        operand: Box::new(local_ref(option, option_ty)),
                        trap_on_none: false,
                    },
                    payload_ty,
                ),
            )],
            else_body: None,
        }));
    }
    let main = h.user_fn("main", hir::Body { locals, statements });
    let executable = h.finish(main);
    let entry = executable.entry();
    let mut source = executable.into_module();
    assert_eq!(
        source.function_types.alloc(hir::FunctionType {
            canonical_type: managed_function,
            is_suspend: false,
            parameter_types: vec![int],
            return_type: int,
        }),
        function_type
    );
    let source = legacy_executable(source, entry);
    let module = lower(&source);
    assert_eq!(module.validate(), Ok(()));

    let body = &module.functions[module.entry].body;
    let mut kinds = Vec::new();
    for (_, block) in body.blocks.iter() {
        let mir::Terminator::Branch {
            cond, then_block, ..
        } = &block.terminator
        else {
            continue;
        };
        let mir::ExprKind::VariantTest { operand, variant } = &cond.kind else {
            continue;
        };
        let mir::ExprKind::Local(tested_local) = &operand.kind else {
            panic!("an Option variant test must consume a stable local")
        };
        assert!(!body.locals[*tested_local].mutable);
        let projection = body.blocks[*then_block]
            .statements
            .iter()
            .find_map(|statement| {
                let mir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                    return None;
                };
                let mir::ExprKind::VariantPayloadProject { operand, field } = &init.kind else {
                    return None;
                };
                Some((init, operand, field))
            })
            .expect("the matching true edge contains the payload projection");
        let mir::ExprKind::Local(projected_local) = &projection.1.kind else {
            panic!("an Option payload projection must consume the tested local")
        };
        assert_eq!(projected_local, tested_local);
        assert_eq!(projection.2.variant(), *variant);
        assert_eq!(
            projection.0.ty,
            projection
                .2
                .definition(&module.enums)
                .expect("the checked field remains valid in the finished MIR module")
                .ty
        );
        kinds.push(projection.0.ty.clone());
    }

    assert_eq!(kinds.len(), 4);
    assert!(kinds.contains(&mir::Type::Integer(mir::IntegerKind::SIGNED_32)));
    assert!(kinds.contains(&mir::Type::String));
    assert!(kinds.iter().any(|ty| matches!(ty, mir::Type::Ptr(_))));
    assert!(kinds.iter().any(|ty| matches!(ty, mir::Type::FunPtr(_))));
}

#[test]
fn elvis_subject_and_rhs_are_each_emitted_once_on_their_own_edges() {
    let mut h = Harness::new();
    let int = h.int;
    let option_int = h.option(int);
    let source = h.user_fn_full(
        "source",
        Vec::new(),
        Vec::new(),
        option_int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(expr(
                    hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 7))),
                    option_int,
                )),
            })],
        },
    );
    let fallback = h.user_fn_full(
        "fallback",
        Vec::new(),
        Vec::new(),
        int,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(int_lit(&h, 9)),
            })],
        },
    );
    let mut locals = Arena::new();
    let subject = locals.alloc(local("$opt", option_int));
    let result = locals.alloc(local("$res", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(subject, call_typed(source, Vec::new(), option_int)),
                stmt(hir::StatementKind::If {
                    cond: expr(
                        hir::ExprKind::IsSome(Box::new(local_ref(subject, option_int))),
                        h.boolean,
                    ),
                    then_body: vec![val_decl(
                        result,
                        expr(
                            hir::ExprKind::Unwrap {
                                operand: Box::new(local_ref(subject, option_int)),
                                trap_on_none: false,
                            },
                            int,
                        ),
                    )],
                    else_body: Some(vec![val_decl(
                        result,
                        call_typed(fallback, Vec::new(), int),
                    )]),
                }),
                expr_stmt(local_ref(result, int)),
            ],
        },
    );
    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));

    let body = &module.functions[module.entry].body;
    let mut source_sites = Vec::new();
    let mut fallback_sites = Vec::new();
    for (block_id, block) in body.blocks.iter() {
        for statement in &block.statements {
            let mir::StatementKind::Call(effect) = &statement.kind else {
                continue;
            };
            let call = match effect {
                mir::CallEffect::Unit(call) => call,
                mir::CallEffect::Value { call, .. } => call,
            };
            let mir::Callee::User(callee) = call.target.callee else {
                continue;
            };
            match module.functions[callee].name.as_str() {
                "source" => source_sites.push(block_id),
                "fallback" => fallback_sites.push(block_id),
                _ => {}
            }
        }
    }
    assert_eq!(source_sites, [body.entry]);
    assert_eq!(fallback_sites.len(), 1);
    assert!(
        body.blocks[fallback_sites[0]].name.starts_with("if.else"),
        "the Elvis RHS must remain short-circuited to the None edge"
    );

    let mir::Terminator::Branch {
        cond, then_block, ..
    } = &body.blocks[body.entry].terminator
    else {
        panic!("the stabilized subject is tested by the Elvis branch")
    };
    let mir::ExprKind::VariantTest {
        operand: tested,
        variant,
    } = &cond.kind
    else {
        panic!("Elvis uses the representation-independent Option test")
    };
    let mir::ExprKind::Local(tested) = tested.kind else {
        panic!("Elvis tests one stable subject local")
    };
    let projection = body.blocks[*then_block]
        .statements
        .iter()
        .find_map(|statement| {
            let mir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                return None;
            };
            let mir::ExprKind::VariantPayloadProject { operand, field } = &init.kind else {
                return None;
            };
            Some((operand, field))
        })
        .expect("the Some edge projects its payload");
    assert!(matches!(projection.0.kind, mir::ExprKind::Local(local) if local == tested));
    assert_eq!(projection.1.variant(), *variant);
}

#[test]
fn generic_enum_unit_constants_preserve_exact_refs_through_concrete_hir_and_mir() {
    let mut h = Harness::new();
    let option_int = h.option(h.int);
    let option_string = h.option(h.string);
    let int_application = h.enum_application_of(option_int);
    let string_application = h.enum_application_of(option_string);
    let none = hir::EnumVariantRef::checked(&h.enums, h.option_enum, 1)
        .expect("test core Option has None");
    let int_none =
        hir::AppliedEnumVariantRef::checked(&h.enums, &h.enum_applications, int_application, none)
            .expect("None belongs to Option<Int>");
    let string_none = hir::AppliedEnumVariantRef::checked(
        &h.enums,
        &h.enum_applications,
        string_application,
        none,
    )
    .expect("None belongs to Option<String>");
    assert_ne!(int_none, string_none);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let executable = h.finish(main);
    let entry = executable.entry();
    let mut export = executable.into_module();
    for (name, ty, variant) in [
        ("noneInt", option_int, int_none),
        ("noneString", option_string, string_none),
    ] {
        export.globals.alloc(hir::Global {
            name: name.to_string(),
            property: hir::PropertyId::from_raw(0.into()),
            ty,
            mutable: false,
            storage: hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::EncodedStaticValue {
                    payload: hir::HirConstantImage::EnumUnit { variant },
                },
            },
            span: SPAN,
        });
    }

    let export = legacy_executable(export, entry);
    let concrete = scoop_hir_lower::concretize_legacy_export(&export);
    let concrete_refs = ["noneInt", "noneString"].map(|name| {
        let global = concrete
            .globals
            .iter()
            .find_map(|(_, global)| (global.name == name).then_some(global))
            .expect("concrete global");
        let hir::concrete::TypeKind::Enum(enum_id) = &concrete.types[global.ty].kind else {
            panic!("constant keeps its exact concrete enum type")
        };
        let hir::concrete::GlobalStorage::Managed {
            state:
                hir::concrete::HirStaticInitialState::EncodedStaticValue {
                    payload: hir::concrete::HirConstantImage::EnumUnit { variant },
                },
        } = &global.storage
        else {
            panic!("constant keeps its checked concrete unit variant")
        };
        assert_eq!(variant.enumeration(), *enum_id);
        assert_eq!(
            concrete.enums[*enum_id].variants[variant.variant().into_raw() as usize].name,
            "None"
        );
        *variant
    });
    assert_ne!(concrete_refs[0], concrete_refs[1]);

    let module = crate::lower(&concrete);
    let mir_refs = ["noneInt", "noneString"].map(|name| {
        let global = module
            .globals
            .iter()
            .find_map(|(_, global)| (global.name == name).then_some(global))
            .expect("MIR global");
        let mir::Type::Enum(enum_id, arguments) = &global.ty else {
            panic!("constant keeps its exact MIR enum type")
        };
        assert_eq!(arguments, &module.enums[*enum_id].type_arguments);
        let mir::GlobalStorage::Managed {
            initial_state:
                mir::MirStaticInitialState::EncodedStaticValue {
                    payload: mir::MirConstantImage::EnumUnit { variant },
                },
        } = &global.storage
        else {
            panic!("constant keeps its checked MIR unit variant")
        };
        assert_eq!(variant.enum_id(), *enum_id);
        assert_eq!(variant.definition(&module.enums).unwrap().name, "None");
        *variant
    });
    assert_ne!(mir_refs[0], mir_refs[1]);
    assert_eq!(
        module.enums[mir_refs[0].enum_id()].type_arguments,
        vec![mir::Type::Integer(mir::IntegerKind::SIGNED_32)]
    );
    assert_eq!(
        module.enums[mir_refs[1].enum_id()].type_arguments,
        vec![mir::Type::String]
    );
    assert_eq!(module.validate(), Ok(()));
}
