use super::*;

fn top_level_property(
    name: &str,
    mutable: bool,
    ty: TypeRef,
    body: ast::PropertyBodySyntax,
) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body,
        span: sp(),
    }
}

fn initializer(expression: Expr, accessors: ast::AccessorSyntax) -> ast::PropertyBodySyntax {
    ast::PropertyBodySyntax::Initializer {
        expression: Box::new(expression),
        accessors,
    }
}

fn integer_with_suffix(magnitude: u64, suffix: ast::IntegerSuffix) -> Expr {
    Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Hexadecimal,
        suffix,
        span: sp(),
    })
}

fn integer_infix(lhs: Expr, name: &str, rhs: Expr) -> Expr {
    Expr::InfixCall {
        lhs: Box::new(lhs),
        target: ast::InfixTarget::Named(ident(name)),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

#[test]
fn ordinary_top_level_properties_own_managed_storage_and_backing_accessors() {
    let seed = ast::PropertyDecl {
        body: ast::PropertyBodySyntax::Const(Box::new(int_lit(40))),
        ..top_level_property(
            "seed",
            false,
            ty_named("Int"),
            initializer(int_lit(0), ast::AccessorSyntax::default()),
        )
    };
    let custom_accessors = ast::AccessorSyntax {
        getter: Some(ast::GetterDecl {
            annotations: Vec::new(),
            body: ast::AccessorBodySyntax::Expr(Box::new(binary(
                BinOp::Add,
                var("field"),
                int_lit(1),
            ))),
            span: sp(),
        }),
        setter: None,
    };
    let source = file(vec![
        Decl::Global(seed),
        Decl::Global(top_level_property(
            "answer",
            false,
            ty_named("Int"),
            initializer(
                binary(BinOp::Add, var("seed"), int_lit(2)),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "label",
            true,
            ty_named("String"),
            initializer(str_lit("ready"), ast::AccessorSyntax::default()),
        )),
        Decl::Global(top_level_property(
            "optional",
            true,
            ty_nullable(ty_named("String")),
            ast::PropertyBodySyntax::OptionalOmitted,
        )),
        Decl::Global(top_level_property(
            "adjusted",
            true,
            ty_named("Int"),
            initializer(int_lit(4), custom_accessors),
        )),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("static top-level properties must lower");

    assert_eq!(module.globals.len(), 4);
    for (global_id, global) in module.globals.iter() {
        assert!(matches!(global.storage, hir::GlobalStorage::Managed { .. }));
        let property = &module.properties[global.property];
        assert!(matches!(
            property.representation,
            hir::PropertyRepresentation::Stored(hir::StoredProperty {
                backing: hir::PropertyBacking::TopLevelGlobal { storage, .. }
            }) if storage == global_id
        ));
    }

    let optional = module
        .globals
        .iter()
        .find_map(|(_, global)| {
            (module.properties[global.property].name == "optional").then_some(global)
        })
        .expect("optional global");
    assert!(matches!(
        optional.storage,
        hir::GlobalStorage::Managed {
            state: hir::HirStaticInitialState::EncodedStaticValue {
                payload: hir::HirConstantImage::EnumUnit { .. }
            }
        }
    ));
    let label = module
        .globals
        .iter()
        .find_map(|(_, global)| {
            (module.properties[global.property].name == "label").then_some(global)
        })
        .expect("label global");
    assert!(matches!(
        &label.storage,
        hir::GlobalStorage::Managed {
            state: hir::HirStaticInitialState::EncodedStaticValue {
                payload: hir::HirConstantImage::String(value)
            }
        } if value == "ready"
    ));

    let adjusted = module
        .properties
        .iter()
        .find_map(|(_, property)| (property.name == "adjusted").then_some(property))
        .expect("adjusted property");
    assert!(matches!(
        module.property_getters[adjusted.capability.getter()].implementation,
        hir::PropertyAccessorImplementation::Body(_)
    ));
}

#[test]
fn typed_integer_intrinsics_are_encoded_in_static_storage() {
    let unsigned = |value| integer_with_suffix(value, ast::IntegerSuffix::Unsigned);
    let long = |value| integer_with_suffix(value, ast::IntegerSuffix::Long);
    let source = file(vec![
        Decl::Global(top_level_property(
            "masked",
            false,
            ty_named("UInt"),
            initializer(
                integer_infix(unsigned(0xf0), "and", unsigned(0x0f)),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "shifted",
            false,
            ty_named("Int"),
            initializer(
                integer_infix(int_lit(1), "shl", long(31)),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "logical",
            false,
            ty_named("Long"),
            initializer(
                integer_infix(unary(UnOp::Neg, int_lit(1)), "ushr", long(1)),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "narrowed",
            false,
            ty_named("UInt8"),
            initializer(
                method_call(int_lit(257), "toUInt8", Vec::new()),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "incremented",
            false,
            ty_named("Int8"),
            initializer(
                method_call(
                    method_call(int_lit(127), "toInt8", Vec::new()),
                    "inc",
                    Vec::new(),
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "compared",
            false,
            ty_named("Long"),
            initializer(
                method_call(
                    method_call(int_lit(1), "toUInt8", Vec::new()),
                    "compareTo",
                    vec![method_call(int_lit(2), "toUInt8", Vec::new())],
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "equal",
            false,
            ty_named("Boolean"),
            initializer(
                method_call(
                    method_call(int_lit(1), "toInt16", Vec::new()),
                    "equals",
                    vec![method_call(int_lit(1), "toInt16", Vec::new())],
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "divided",
            false,
            ty_named("Int8"),
            initializer(
                method_call(
                    method_call(unary(UnOp::Neg, int_lit(128)), "toInt8", Vec::new()),
                    "div",
                    vec![method_call(
                        unary(UnOp::Neg, int_lit(1)),
                        "toInt8",
                        Vec::new(),
                    )],
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "remainder",
            false,
            ty_named("ULong"),
            initializer(
                method_call(
                    method_call(unsigned(u64::MAX), "toUInt64", Vec::new()),
                    "rem",
                    vec![method_call(unsigned(2), "toUInt64", Vec::new())],
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "jointWide",
            false,
            ty_named("Boolean"),
            initializer(
                binary(
                    BinOp::Eq,
                    int_lit(1),
                    Expr::IntLiteral(ast::IntegerLiteralSyntax {
                        magnitude: 2_147_483_648,
                        radix: ast::IntegerRadix::Decimal,
                        suffix: ast::IntegerSuffix::None,
                        span: sp(),
                    }),
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("typed integer static constants must lower");
    let payload = |name| {
        module
            .globals
            .iter()
            .find_map(|(_, global)| {
                (module.properties[global.property].name == name).then(|| match &global.storage {
                    hir::GlobalStorage::Managed {
                        state:
                            hir::HirStaticInitialState::EncodedStaticValue {
                                payload: hir::HirConstantImage::Integer(value),
                            },
                    } => *value,
                    _ => panic!("{name} must have an encoded integer static value"),
                })
            })
            .unwrap_or_else(|| panic!("missing global {name}"))
    };

    assert_eq!(payload("masked"), hir::HirIntegerConstant::Unsigned32(0));
    assert_eq!(
        payload("shifted"),
        hir::HirIntegerConstant::Signed32(0x8000_0000)
    );
    assert_eq!(
        payload("logical"),
        hir::HirIntegerConstant::Signed64(i64::MAX as u64)
    );
    assert_eq!(payload("narrowed"), hir::HirIntegerConstant::Unsigned8(1));
    assert_eq!(
        payload("incremented"),
        hir::HirIntegerConstant::Signed8(0x80)
    );
    assert_eq!(
        payload("compared"),
        hir::HirIntegerConstant::Signed64(u64::MAX)
    );
    assert_eq!(payload("divided"), hir::HirIntegerConstant::Signed8(0x80));
    assert_eq!(payload("remainder"), hir::HirIntegerConstant::Unsigned64(1));
    let equal = module
        .globals
        .iter()
        .find_map(|(_, global)| {
            (module.properties[global.property].name == "equal").then_some(global)
        })
        .expect("missing global equal");
    assert!(matches!(
        equal.storage,
        hir::GlobalStorage::Managed {
            state: hir::HirStaticInitialState::EncodedStaticValue {
                payload: hir::HirConstantImage::Boolean(true),
            },
        }
    ));
    let joint_wide = module
        .globals
        .iter()
        .find_map(|(_, global)| {
            (module.properties[global.property].name == "jointWide").then_some(global)
        })
        .expect("missing global jointWide");
    assert!(matches!(
        joint_wide.storage,
        hir::GlobalStorage::Managed {
            state: hir::HirStaticInitialState::EncodedStaticValue {
                payload: hir::HirConstantImage::Boolean(false),
            },
        }
    ));
    assert!(module.initialization_units.is_empty());
}

#[test]
fn static_integer_div_rem_zero_uses_runtime_initialization() {
    let int8 = |value| method_call(int_lit(value), "toInt8", Vec::new());
    let ulong = |value| {
        method_call(
            integer_with_suffix(value, ast::IntegerSuffix::Unsigned),
            "toUInt64",
            Vec::new(),
        )
    };
    let source = file(vec![
        Decl::Global(top_level_property(
            "zeroDiv",
            false,
            ty_named("Int8"),
            initializer(
                method_call(int8(1), "div", vec![int8(0)]),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "zeroRem",
            false,
            ty_named("ULong"),
            initializer(
                method_call(ulong(1), "rem", vec![ulong(0)]),
                ast::AccessorSyntax::default(),
            ),
        )),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("ordinary div/rem by zero remains a runtime operation");
    assert_eq!(module.initialization_units.len(), 2);
    for name in ["zeroDiv", "zeroRem"] {
        let global = module
            .globals
            .iter()
            .find_map(|(_, global)| {
                (module.properties[global.property].name == name).then_some(global)
            })
            .unwrap_or_else(|| panic!("missing global {name}"));
        assert!(matches!(
            global.storage,
            hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::ZeroedForRuntimeUnit { .. },
            }
        ));
    }
}

#[test]
fn static_integer_conversions_encode_width_and_signedness_boundaries() {
    struct Case {
        name: &'static str,
        ty: &'static str,
        expression: Expr,
        expected: hir::HirIntegerConstant,
    }

    let converted = |expression, source: &str, target: &str| {
        method_call(
            method_call(expression, source, Vec::new()),
            target,
            Vec::new(),
        )
    };
    let signed = |magnitude| unary(UnOp::Neg, int_lit(magnitude));
    let signed_long = |magnitude| {
        unary(
            UnOp::Neg,
            integer_with_suffix(magnitude, ast::IntegerSuffix::Long),
        )
    };
    let unsigned = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::Unsigned);
    let cases = [
        Case {
            name: "int8ToULong",
            ty: "ULong",
            expression: converted(signed(1), "toInt8", "toUInt64"),
            expected: hir::HirIntegerConstant::Unsigned64(u64::MAX),
        },
        Case {
            name: "int16ToUInt8",
            ty: "UInt8",
            expression: converted(signed(257), "toInt16", "toUInt8"),
            expected: hir::HirIntegerConstant::Unsigned8(0xff),
        },
        Case {
            name: "intToLong",
            ty: "Long",
            expression: converted(signed(65_537), "toInt32", "toInt64"),
            expected: hir::HirIntegerConstant::Signed64(0xffff_ffff_fffe_ffff),
        },
        Case {
            name: "longToUInt",
            ty: "UInt",
            expression: converted(signed_long(4_294_967_297), "toInt64", "toUInt32"),
            expected: hir::HirIntegerConstant::Unsigned32(0xffff_ffff),
        },
        Case {
            name: "uint8ToLong",
            ty: "Long",
            expression: converted(unsigned(0xff), "toUInt8", "toInt64"),
            expected: hir::HirIntegerConstant::Signed64(0xff),
        },
        Case {
            name: "uint16ToInt8",
            ty: "Int8",
            expression: converted(unsigned(0xffff), "toUInt16", "toInt8"),
            expected: hir::HirIntegerConstant::Signed8(0xff),
        },
        Case {
            name: "uintToULong",
            ty: "ULong",
            expression: converted(unsigned(0xffff_ffff), "toUInt32", "toUInt64"),
            expected: hir::HirIntegerConstant::Unsigned64(0xffff_ffff),
        },
        Case {
            name: "ulongToInt",
            ty: "Int",
            expression: converted(unsigned(u64::MAX), "toUInt64", "toInt32"),
            expected: hir::HirIntegerConstant::Signed32(0xffff_ffff),
        },
    ];

    let mut declarations = cases
        .iter()
        .map(|case| {
            Decl::Global(top_level_property(
                case.name,
                false,
                ty_named(case.ty),
                initializer(case.expression.clone(), ast::AccessorSyntax::default()),
            ))
        })
        .collect::<Vec<_>>();
    declarations.push(fun("main", Vec::new()));
    let module = lower_user(file(declarations))
        .expect("representative static integer conversions must fold");

    for case in cases {
        let value = module
            .globals
            .iter()
            .find_map(|(_, global)| {
                (module.properties[global.property].name == case.name).then(|| {
                    match global.storage {
                        hir::GlobalStorage::Managed {
                            state:
                                hir::HirStaticInitialState::EncodedStaticValue {
                                    payload: hir::HirConstantImage::Integer(value),
                                },
                        } => value,
                        _ => panic!("{} must have encoded integer storage", case.name),
                    }
                })
            })
            .unwrap_or_else(|| panic!("missing global {}", case.name));
        assert_eq!(value, case.expected, "wrong value for {}", case.name);
    }
    assert!(module.initialization_units.is_empty());
}

#[test]
fn static_integer_folding_does_not_contextualize_composite_receivers() {
    let source = file(vec![
        Decl::Global(top_level_property(
            "nestedOperator",
            false,
            ty_named("Int8"),
            initializer(
                binary(
                    BinOp::Add,
                    binary(BinOp::Add, int_lit(1), int_lit(2)),
                    int_lit(3),
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "nestedMethod",
            false,
            ty_named("Int8"),
            initializer(
                method_call(
                    binary(BinOp::Add, int_lit(1), int_lit(2)),
                    "inc",
                    Vec::new(),
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source)
        .expect_err("static folding must preserve ordinary receiver inference boundaries");
    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                **message == "top-level property initializer must be of type Int8, found Int"
            })
            .count(),
        2,
        "unexpected diagnostics: {messages:?}"
    );
}

#[test]
fn failed_static_integer_probe_does_not_poison_extension_fallback() {
    let mut base = top_level_property(
        "base",
        false,
        ty_named("Int8"),
        initializer(int_lit(0), ast::AccessorSyntax::default()),
    );
    base.body = ast::PropertyBodySyntax::Const(Box::new(int_lit(1)));
    let source = file(vec![
        Decl::Global(base),
        extension_expr(
            ty_named("Int8"),
            "plus",
            Vec::new(),
            vec![("other", ty_named("Long"))],
            Some(ty_named("Int8")),
            this_expr(),
        ),
        Decl::Global(top_level_property(
            "runtime",
            false,
            ty_named("Int8"),
            initializer(
                method_call(
                    var("base"),
                    "plus",
                    vec![integer_with_suffix(2, ast::IntegerSuffix::Long)],
                ),
                ast::AccessorSyntax::default(),
            ),
        )),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source)
        .expect("a failed intrinsic image probe must leave extension resolution untouched");
    let runtime = module
        .globals
        .iter()
        .find_map(|(_, global)| {
            (module.properties[global.property].name == "runtime").then_some(global)
        })
        .expect("missing runtime global");
    assert!(matches!(
        runtime.storage,
        hir::GlobalStorage::Managed {
            state: hir::HirStaticInitialState::ZeroedForRuntimeUnit { .. },
        }
    ));
    assert_eq!(module.initialization_units.len(), 1);
}

#[test]
fn non_static_top_level_initializer_is_kept_out_of_image_storage() {
    let source = file(vec![
        Decl::Global(top_level_property(
            "value",
            false,
            ty_named("Int"),
            initializer(call("make", Vec::new()), ast::AccessorSyntax::default()),
        )),
        fun_expr(
            "make",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("a runtime initializer must form a typed init unit");
    assert_eq!(module.initialization_units.len(), 1);
    assert_eq!(module.initialization_failure_roots.len(), 1);

    let (unit_id, unit) = module
        .initialization_units
        .iter()
        .next()
        .expect("runtime initialization unit");
    let hir::InitializationUnitKind::EagerTopLevel { property, storage } = unit.kind else {
        panic!("runtime top-level property must own eager-top-level initialization")
    };
    assert_eq!(unit.schedule, hir::InitializationSchedule::EagerStartup);
    assert_eq!(unit.stable_key, "$local$u0:p0:r9:top-leveln5:value");
    assert_eq!(unit.display_name, "top-level:value");
    assert!(unit.dependencies.is_empty());
    assert!(matches!(
        module.globals[storage].storage,
        hir::GlobalStorage::Managed {
            state: hir::HirStaticInitialState::ZeroedForRuntimeUnit { unit: id }
        } if id == unit_id
    ));
    assert!(matches!(
        module.properties[property].representation,
        hir::PropertyRepresentation::Stored(hir::StoredProperty {
            backing: hir::PropertyBacking::TopLevelGlobal {
                storage: actual_storage,
                initialization: hir::TopLevelInitialization::Runtime(id),
            },
        }) if actual_storage == storage && id == unit_id
    ));

    let hir::FunctionKind::User(initializer) = &module.functions[unit.initializer].kind else {
        panic!("runtime initializer must be an ordinary body");
    };
    assert!(matches!(
        initializer.statements.last().map(|statement| &statement.kind),
        Some(hir::StatementKind::Assign {
            target: hir::AssignTarget::Global(actual_storage),
            ..
        }) if *actual_storage == storage
    ));

    let getter = module.properties[property].capability.getter();
    let hir::PropertyAccessorImplementation::Body(getter) =
        module.property_getters[getter].implementation
    else {
        panic!("runtime-backed getter must be a body");
    };
    let hir::FunctionKind::User(getter) = &module.functions[getter].kind else {
        panic!("runtime-backed getter must be an ordinary function");
    };
    assert!(matches!(
        getter.statements.first().map(|statement| &statement.kind),
        Some(hir::StatementKind::InitializationEnsure(id)) if *id == unit_id
    ));
}

#[test]
fn direct_top_level_reads_form_typed_dependencies() {
    let source = file(vec![
        Decl::Global(top_level_property(
            "alpha",
            false,
            ty_named("Int"),
            initializer(
                binary(BinOp::Add, var("zed"), int_lit(2)),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "zed",
            false,
            ty_named("Int"),
            initializer(call("make", Vec::new()), ast::AccessorSyntax::default()),
        )),
        fun_expr(
            "make",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(40),
        ),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("direct runtime dependency must lower");
    assert_eq!(module.initialization_units.len(), 2);

    let (alpha_id, alpha) = module
        .initialization_units
        .iter()
        .find(|(_, unit)| unit.stable_key == "$local$u0:p0:r9:top-leveln5:alpha")
        .expect("alpha unit");
    let (zed_id, zed) = module
        .initialization_units
        .iter()
        .find(|(_, unit)| unit.stable_key == "$local$u0:p0:r9:top-leveln3:zed")
        .expect("zed unit");
    assert_eq!(
        alpha
            .dependencies
            .iter()
            .map(|dependency| dependency.unit)
            .collect::<Vec<_>>(),
        vec![zed_id]
    );
    assert!(zed.dependencies.is_empty());
    assert_ne!(alpha_id, zed_id);
}

#[test]
fn direct_top_level_dependency_cycle_is_a_stable_hir_error() {
    let source = file(vec![
        Decl::Global(top_level_property(
            "left",
            false,
            ty_named("Int"),
            initializer(var("right"), ast::AccessorSyntax::default()),
        )),
        Decl::Global(top_level_property(
            "right",
            false,
            ty_named("Int"),
            initializer(var("left"), ast::AccessorSyntax::default()),
        )),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source).expect_err("direct dependency cycle must be rejected");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message
            == "initialization cycle: top-level:left -> top-level:right -> top-level:left"
    }));
}

#[test]
fn private_top_level_dependency_cycle_uses_a_source_display_name() {
    let private = |name, expression| {
        let mut property = top_level_property(
            name,
            false,
            ty_named("Int"),
            initializer(expression, ast::AccessorSyntax::default()),
        );
        property.visibility = ast::VisibilitySyntax::Explicit {
            visibility: ast::DeclaredVisibility::Private,
            span: sp(),
        };
        Decl::Global(property)
    };
    let source = file(vec![
        private("left", var("right")),
        private("right", var("left")),
        fun("main", Vec::new()),
    ]);

    let errors = lower_user(source).expect_err("a private dependency cycle must be rejected");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message
            == "initialization cycle: top-level-private:<user>:left -> top-level-private:<user>:right -> top-level-private:<user>:left"
    }));
}
