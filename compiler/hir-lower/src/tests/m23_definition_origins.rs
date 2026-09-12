use std::collections::HashSet;

use scoop_identity::DefinitionOriginSubject;

use super::*;

fn type_alias(name: &str, target: TypeRef) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        target,
        span: sp(),
    })
}

fn runtime_property(name: &str) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(call("initialValue", Vec::new())),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

fn extern_function(name: &str) -> Decl {
    let annotation = ast::Annotation {
        name: ident("Extern"),
        args: [
            ("lib", "numbers"),
            ("name", "native_identity"),
            ("abi", "c"),
        ]
        .into_iter()
        .map(|(parameter, value)| ast::AnnotationArg {
            name: Some(ident(parameter)),
            value: ast::AnnotationLiteral::String(value.to_string()),
            span: sp(),
        })
        .collect(),
        span: sp(),
    };
    let mut declaration = fun_sig(
        name,
        Vec::new(),
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        Vec::new(),
    );
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.annotations.push(annotation);
    function.body = ast::FunctionBody::None;
    declaration
}

fn callback_registration() -> ast::Expr {
    let signature = ty_function(
        false,
        vec![ty_named("Int"), ty_generic("Ptr", vec![ty_named("Unit")])],
        ty_named("Int"),
    );
    let callback = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: Some(ty_named("Int")),
            span: sp(),
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    };
    typed_source_call(
        "foreignCallback",
        vec![signature],
        vec![
            named_argument("mode", field(var("ForeignCallbackMode"), "Reusable")),
            named_argument("callback", callback),
            named_argument("contextIndex", int_lit(1)),
        ],
    )
}

fn fixture() -> ast::SourceFile {
    file(vec![
        type_alias("Count", ty_named("Int")),
        struct_decl("Packet", vec![("code", ty_named("Int"))]),
        enum_decl(
            "Choice",
            Vec::new(),
            vec![
                variant_unit("Empty"),
                variant_named("Number", vec![("value", ty_named("Int"))]),
            ],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Boxed",
            vec![(false, "value", ty_named("Int"))],
            None,
            Vec::new(),
            Vec::new(),
        ),
        fun_sig(
            "initialValue",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            vec![ret(Some(int_lit(7)))],
        ),
        runtime_property("seed"),
        extern_function("nativeIdentity"),
        fun(
            "main",
            vec![unsafe_block(vec![val(
                "registered",
                callback_registration(),
            )])],
        ),
    ])
}

#[test]
fn export_definition_origins_cover_every_export_subject_exactly_once() {
    let output = lower_user_output(fixture()).expect("the origin coverage fixture must lower");
    let module = output.export.module();
    let mut expected = expected_subjects(module);
    expected.sort_by(|left, right| left.compare_sort_key(*right));
    let unique = expected.iter().copied().collect::<HashSet<_>>();
    assert_eq!(unique.len(), expected.len());
    assert_eq!(module.export_definition_origins.len(), expected.len());

    for subject in expected {
        let record = module
            .export_definition_origins
            .get(subject)
            .unwrap_or_else(|| panic!("missing definition origin for {subject:?}"));
        let context = module
            .source_context_identities
            .iter()
            .find(|context| context.id() == record.origin().context())
            .unwrap_or_else(|| panic!("origin context is absent for {subject:?}"));
        assert_eq!(context.key().source(), record.origin().source());
        assert!(
            module
                .source_files
                .iter()
                .any(|source| &source.identity == record.origin().source())
        );
    }

    assert!(
        module
            .export_definition_origins
            .records()
            .windows(2)
            .all(|pair| pair[0]
                .subject()
                .compare_sort_key(pair[1].subject())
                .is_lt())
    );
    assert!(
        module
            .export_definition_origins
            .records()
            .iter()
            .any(|record| matches!(
                record.subject(),
                DefinitionOriginSubject::CallbackRegistration(_)
            ))
    );
    assert!(
        module
            .export_definition_origins
            .records()
            .iter()
            .any(|record| matches!(
                record.subject(),
                DefinitionOriginSubject::SourceNativeContract(_)
            ))
    );
    assert!(
        module
            .export_definition_origins
            .records()
            .iter()
            .any(|record| matches!(
                record.subject(),
                DefinitionOriginSubject::InitializationUnit(_)
            ))
    );
}

#[test]
fn struct_field_property_and_accessor_keep_the_field_source_span() {
    let mut declaration = struct_decl(
        "SourceSpans",
        vec![("left", ty_named("Int")), ("right", ty_named("Int"))],
    );
    let Decl::Struct(structure) = &mut declaration else {
        unreachable!()
    };
    let ast::StructRepresentationDecl::Declared(fields) = &mut structure.fields else {
        unreachable!()
    };
    fields[0].span = Span::new(10, 14);
    fields[1].span = Span::new(20, 25);
    let output = lower_user_output(file(vec![declaration, fun("main", Vec::new())]))
        .expect("the source-span fixture must lower");
    let module = output.export.module();
    let (structure, declaration) = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "SourceSpans")
        .expect("the source struct is present");

    for (index, expected) in [(0_u32, (10_u64, 14_u64)), (1, (20, 25))] {
        let field = hir::StructFieldRef::checked(&module.structs, structure, index)
            .expect("the field belongs to the source struct");
        let property = declaration.properties[index as usize];
        let accessor = module.properties[property].capability.getter();
        for subject in [
            DefinitionOriginSubject::Field(module.field_identities[field].id()),
            match &module.property_identities[property] {
                hir::HirPropertyIdentity::Ordinary(record) => {
                    DefinitionOriginSubject::Property(record.id())
                }
                hir::HirPropertyIdentity::Extension(_) => {
                    panic!("a struct field is an ordinary property")
                }
            },
            DefinitionOriginSubject::PropertyAccessor(
                module.property_accessor_identities[accessor].id(),
            ),
        ] {
            let span = module
                .export_definition_origins
                .get(subject)
                .expect("the source-backed subject has one origin")
                .origin()
                .span();
            assert_eq!((span.start_byte(), span.end_byte()), expected);
        }
    }
}

fn expected_subjects(module: &hir::Module) -> Vec<DefinitionOriginSubject> {
    let mut subjects = Vec::new();
    for (id, _) in module.structs.iter() {
        append_nominal(&mut subjects, &module.nominal_identities[id]);
    }
    for (id, _) in module.enums.iter() {
        append_nominal(&mut subjects, &module.nominal_identities[id]);
    }
    for (id, _) in module.classes.iter() {
        append_nominal(&mut subjects, &module.nominal_identities[id]);
    }
    for (id, _) in module.interfaces.iter() {
        append_nominal(&mut subjects, &module.nominal_identities[id]);
    }
    for (id, _) in module.objects.iter() {
        append_nominal(&mut subjects, &module.nominal_identities[id]);
    }
    for (id, _) in module.functions.iter() {
        if let Some(identity) = module.function_identities[id].source_identity() {
            subjects.push(match identity {
                hir::HirSourceFunctionIdentity::Plain(record) => {
                    DefinitionOriginSubject::Function(record.id())
                }
                hir::HirSourceFunctionIdentity::Generic(record) => {
                    DefinitionOriginSubject::GenericFunction(record.id())
                }
            });
        }
    }
    for (id, _) in module.struct_constructors.iter() {
        subjects.push(DefinitionOriginSubject::Constructor(
            module.constructor_identities[id].id(),
        ));
    }
    for (id, _) in module.class_constructors.iter() {
        if let hir::HirClassConstructorIdentity::Source(record) = &module.constructor_identities[id]
        {
            subjects.push(DefinitionOriginSubject::Constructor(record.id()));
        }
    }
    for (id, property) in module.properties.iter() {
        subjects.push(match &module.property_identities[id] {
            hir::HirPropertyIdentity::Ordinary(record) => {
                DefinitionOriginSubject::Property(record.id())
            }
            hir::HirPropertyIdentity::Extension(record) => {
                DefinitionOriginSubject::ExtensionProperty(record.id())
            }
        });
        let getter = module.property_accessor_identities[property.capability.getter()].id();
        subjects.push(DefinitionOriginSubject::PropertyAccessor(getter));
        if let Some(setter) = property.capability.setter() {
            subjects.push(DefinitionOriginSubject::PropertyAccessor(
                module.property_accessor_identities[setter].id(),
            ));
        }
    }
    for (id, _) in module.type_aliases.iter() {
        subjects.push(DefinitionOriginSubject::TypeAlias(
            module.type_alias_identities[id].id(),
        ));
    }
    for (structure, declaration) in module.structs.iter() {
        for index in 0..declaration.semantic_fields().len() {
            let field = hir::StructFieldRef::checked(&module.structs, structure, index as u32)
                .expect("the field is checked against its declaration");
            subjects.push(DefinitionOriginSubject::Field(
                module.field_identities[field].id(),
            ));
        }
    }
    for (field, _) in module.class_fields.iter() {
        subjects.push(DefinitionOriginSubject::Field(
            module.field_identities[field].id(),
        ));
    }
    for (enumeration, declaration) in module.enums.iter() {
        for variant_index in 0..declaration.variants.len() {
            let variant =
                hir::EnumVariantRef::checked(&module.enums, enumeration, variant_index as u32)
                    .expect("the variant is checked against its declaration");
            subjects.push(DefinitionOriginSubject::EnumVariant(
                module.enum_member_identities[variant].id(),
            ));
            for field_index in 0..declaration.variants[variant_index].fields.len() {
                let field =
                    hir::EnumVariantFieldRef::checked(&module.enums, variant, field_index as u32)
                        .expect("the field is checked against its variant");
                subjects.push(DefinitionOriginSubject::EnumVariantField(
                    module.enum_member_identities[field].id(),
                ));
            }
        }
    }
    subjects.extend(
        module
            .initialization_unit_identities
            .records()
            .iter()
            .map(|record| DefinitionOriginSubject::InitializationUnit(record.id())),
    );
    subjects.extend(
        module
            .local_binding_identities
            .iter()
            .map(|identity| DefinitionOriginSubject::LocalBinding(identity.record().id())),
    );
    subjects.extend(
        module
            .callback_registration_identities
            .records()
            .iter()
            .map(|record| DefinitionOriginSubject::CallbackRegistration(record.id())),
    );
    subjects.extend(
        module
            .source_native_contracts
            .iter()
            .map(|contract| DefinitionOriginSubject::SourceNativeContract(contract.record().id())),
    );
    subjects
}

fn append_nominal(subjects: &mut Vec<DefinitionOriginSubject>, identity: &hir::HirNominalIdentity) {
    let Some(identity) = identity.source() else {
        return;
    };
    subjects.push(match identity {
        hir::HirSourceNominalIdentity::Concrete(record) => {
            DefinitionOriginSubject::Type(record.id())
        }
        hir::HirSourceNominalIdentity::Generic(record) => {
            DefinitionOriginSubject::GenericType(record.id())
        }
    });
}
