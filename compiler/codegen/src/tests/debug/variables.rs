use super::*;
use scoop_lir::{
    DebugBinding, DebugEncoding, DebugField, DebugSignature, DebugStorage, DebugType,
    DebugTypeKind, DebugTypeRef,
};
use scoop_source::{DebugDeclaration, DebugOrigin};

pub(super) fn origin(line: u32) -> DebugOrigin {
    DebugRecords::default().source(
        Arc::new(DebugFile {
            identity: Arc::new(scoop_identity::SourceIdentity::single_file()),
            path: "/workspace/variables.scoop".into(),
        }),
        line,
        5,
    )
}

pub(super) fn bind(
    function: &mut Function,
    name: &str,
    ty: PersistentExactTypeId,
    storage: DebugStorage,
    parameter: Option<u32>,
) {
    let scoop_identity::CallableBodyKeyKind::Strong(
        scoop_identity::StrongCallableDefinitionOwner::Function(owner),
    ) = function.callable_body.identity_record().key().kind()
    else {
        panic!("this fixture uses a source function");
    };
    let selector = match parameter {
        Some(index) => scoop_identity::LocalValueSelector::Parameter {
            declaration_index: index - 1,
        },
        None => scoop_identity::LocalValueSelector::LocalDeclaration {
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(
                    StructuralDefinitionSiteRole::LocalDeclaration,
                    function.debug_variables.len() as u32,
                ),
                [],
            ),
        },
    };
    let key = scoop_identity::LocalValueKey::new(
        scoop_identity::CallableMaterialization::new(
            scoop_identity::CallableTemplateOwner::Function(owner),
            CallableMaterializationContext::NoSubstitution,
        ),
        selector,
    );
    let DebugOrigin::Source(location) = origin(3) else {
        unreachable!()
    };
    function.debug_variables.push(DebugBinding {
        declaration: Arc::new(DebugDeclaration {
            identity: scoop_identity::PersistentLocalValueId::from_key(&key).unwrap(),
            name: name.into(),
        }),
        location,
        ty,
        storage,
        managed: match storage {
            DebugStorage::Local(local) => match function.locals[local].storage() {
                scoop_lir::LocalStorage::NonZero(value) => value.scan().contains_reference(),
                _ => false,
            },
            DebugStorage::Parameter(index) => function.signature.arguments()[index as usize]
                .scan()
                .contains_reference(),
            DebugStorage::Temporary(temp) => function.temps[temp].ty == MANAGED_PTR,
        },
        parameter: parameter.map(|index| std::num::NonZeroU32::new(index).unwrap()),
    });
}

fn field(name: &str, offset: u64, ty: DebugTypeRef) -> DebugField {
    DebugField {
        name: name.into(),
        offset,
        ty,
        artificial: false,
    }
}

#[test]
fn full_debug_emits_source_variables_recursive_layouts_and_abi_fragments() {
    let mut module = values_module();
    let long = test_exact_type("debug.Long");
    let point = test_exact_type("debug.Point");
    let unit = test_exact_type("debug.Unit");
    let node = test_exact_type("debug.Node");
    let choice = test_exact_type("debug.Choice");
    let callback = test_exact_type("debug.Callback");
    let interface = test_exact_type("debug.Interface");
    module.meta.debug_types.extend([
        (
            long,
            DebugType {
                name: "Long".into(),
                size: 8,
                align: 8,
                kind: DebugTypeKind::Basic(DebugEncoding::Signed),
            },
        ),
        (
            point,
            DebugType {
                name: "Point".into(),
                size: 16,
                align: 8,
                kind: DebugTypeKind::Record(vec![
                    field("x", 0, DebugTypeRef::Source(long)),
                    field("y", 8, DebugTypeRef::Source(long)),
                ]),
            },
        ),
        (
            unit,
            DebugType {
                name: "Unit".into(),
                size: 0,
                align: 1,
                kind: DebugTypeKind::Record(Vec::new()),
            },
        ),
        (
            node,
            DebugType {
                name: "Node".into(),
                size: 8,
                align: 8,
                kind: DebugTypeKind::Pointer(DebugTypeRef::Inline(Box::new(DebugType {
                    name: "NodeObject".into(),
                    size: 16,
                    align: 8,
                    kind: DebugTypeKind::Record(vec![
                        field("next", 0, DebugTypeRef::Source(node)),
                        field("value", 8, DebugTypeRef::Source(long)),
                    ]),
                }))),
            },
        ),
        (
            choice,
            DebugType {
                name: "Choice".into(),
                size: 1,
                align: 1,
                kind: DebugTypeKind::Enumeration {
                    signed: true,
                    variants: vec![("Negative".into(), 255), ("Positive".into(), 1)],
                },
            },
        ),
        (
            callback,
            DebugType {
                name: "Callback".into(),
                size: 8,
                align: 8,
                kind: DebugTypeKind::Pointer(DebugTypeRef::Inline(Box::new(DebugType {
                    name: String::new(),
                    size: 0,
                    align: 1,
                    kind: DebugTypeKind::Function {
                        parameters: vec![DebugTypeRef::Source(long)],
                        result: Some(DebugTypeRef::Source(long)),
                    },
                }))),
            },
        ),
        (
            interface,
            DebugType {
                name: "Interface".into(),
                size: 16,
                align: 8,
                kind: DebugTypeKind::Record(vec![
                    field("object", 0, DebugTypeRef::Source(node)),
                    field("table", 8, DebugTypeRef::Source(long)),
                ]),
            },
        ),
    ]);
    let function = &mut module.functions[0];
    function.debug_name = "inspect".into();
    function.debug_origin = origin(1);
    function.signature = plain_scoop_signature(
        vec![MANAGED_PTR, LirType::I8, RAW_PTR, LirType::Interface],
        LirType::Void,
    );
    function.debug_signature = DebugSignature::Source {
        parameters: vec![node, choice, callback, interface],
        result: None,
    };
    let locals = function.locals.iter().map(|(id, _)| id).collect::<Vec<_>>();
    for ((id, name), ty) in locals
        .into_iter()
        .zip(["number", "point", "unit"])
        .zip([long, point, unit])
    {
        bind(function, name, ty, DebugStorage::Local(id), None);
    }
    for (index, (name, ty)) in [
        ("head", node),
        ("choice", choice),
        ("callback", callback),
        ("view", interface),
    ]
    .into_iter()
    .enumerate()
    {
        bind(
            function,
            name,
            ty,
            DebugStorage::Parameter(index as u32),
            Some(index as u32 + 1),
        );
    }
    let profile = host_profile().with_debug_info(scoop_lir::DebugInfoLevel::Full);
    let machine = profile.create_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
    llvm.verify().unwrap();
    let ir = llvm.print_to_string().to_string();
    for expected in [
        "#dbg_declare(ptr %n,",
        "#dbg_declare(ptr %p,",
        "!DILocalVariable(name: \"number\"",
        "!DILocalVariable(name: \"view\", arg: 4",
        "DW_OP_LLVM_fragment, 0, 64",
        "DW_OP_LLVM_fragment, 64, 64",
        "name: \"Negative\", value: -1",
        "name: \"NodeObject\"",
        "name: \"next\"",
        "name: \"y\"",
        "name: \"int8_t\", size: 8",
        "!DISubroutineType(",
    ] {
        assert!(ir.contains(expected), "missing {expected}: {ir}");
    }
    let expected = statepoint::expectations(&module).unwrap();
    statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
    llvm.verify().unwrap();
    let directory = tempfile::tempdir().unwrap();
    machine
        .write_to_file(
            &llvm,
            FileType::Object,
            &directory.path().join("variables.o"),
        )
        .unwrap();
}
