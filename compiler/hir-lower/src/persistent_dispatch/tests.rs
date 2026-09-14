use std::collections::BTreeMap;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{CborIdentityRecord, DispatchRole, DispatchSlotKey};

use crate::tests::{
    bodyless_method, class_decl, core_file, core_source_identity, file, fun, int_lit, method_expr,
    override_method_expr, sp, test_source_identity, ty_named, with_method_modifier,
};

fn interface_property(name: &str) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: crate::tests::ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Abstract,
        span: sp(),
    }
}

fn interface(name: &str) -> ast::Decl {
    ast::Decl::Interface(ast::InterfaceDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: crate::tests::ident(name),
        type_params: Vec::new(),
        supertypes: Vec::new(),
        where_clause: None,
        methods: vec![bodyless_method(
            false,
            "run",
            Vec::new(),
            Some(ty_named("Int")),
        )],
        properties: vec![interface_property("value")],
        nested: Vec::new(),
        companion: None,
        span: sp(),
    })
}

fn lower(mut declarations: Vec<ast::Decl>, unrelated_prefix: bool) -> hir::Output {
    if unrelated_prefix {
        declarations.insert(0, fun("unrelated", Vec::new()));
    }
    let core = core_file();
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(
            test_source_identity("src/dispatch.scoop"),
            file(declarations),
        ),
        Vec::new(),
    ))
    .expect("dispatch identity test source is valid");
    let input = crate::LegacyCombinedSources::try_new(
        vec![crate::ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::CurrentSourceDetails {
            display_locator: "/checkout/dispatch.scoop",
            source_text: "",
        },
    )
    .expect("dispatch identity test sources are valid");
    crate::lower_combined_sources(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("dispatch identity fixture lowers")
}

fn virtual_fixture(unrelated_prefix: bool) -> hir::Output {
    let base = with_method_modifier(
        method_expr("value", Vec::new(), Some(ty_named("Int")), int_lit(1)),
        ast::MethodModifier::Open,
    );
    lower(
        vec![
            class_decl(
                ast::ClassModifier::Open,
                "Base",
                Vec::new(),
                None,
                Vec::new(),
                vec![base],
            ),
            class_decl(
                ast::ClassModifier::Final,
                "Derived",
                Vec::new(),
                Some(("Base", Vec::new())),
                Vec::new(),
                vec![override_method_expr(
                    "value",
                    Vec::new(),
                    Some(ty_named("Int")),
                    int_lit(2),
                )],
            ),
            fun("main", Vec::new()),
        ],
        unrelated_prefix,
    )
}

fn function_named(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn concrete_function_named(
    module: &hir::LocalConcreteHir,
    name: &str,
) -> hir::concrete::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing concrete function `{name}`"))
}

fn plain_function_record(
    module: &hir::Module,
    function: hir::FunctionId,
) -> &hir::HirPlainFunctionIdentity {
    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) =
        &module.function_identities[function]
    else {
        panic!("dispatch method must have a plain source identity")
    };
    record
}

#[test]
fn virtual_override_family_uses_the_root_declaration_identity() {
    let output = virtual_fixture(false);
    let module = &output.export;
    let base = function_named(module, "Base.value");
    let derived = function_named(module, "Derived.value");
    let hir::MethodDispatch::Virtual(family) = module.functions[base]
        .method
        .expect("base method metadata")
        .dispatch
    else {
        panic!("the open base method must own a virtual family")
    };
    assert_eq!(
        module.functions[derived]
            .method
            .expect("derived method metadata")
            .dispatch,
        hir::MethodDispatch::FinalOverride(family)
    );
    assert_eq!(
        module.dispatch_slot_identities.virtual_root(family),
        Some(base)
    );
    assert_eq!(
        module.dispatch_slot_identities[family].key(),
        &DispatchSlotKey::virtual_method(plain_function_record(module, base).id())
    );
    let concrete_base = concrete_function_named(&output.local, "Base.value");
    let hir::concrete::MethodDispatch::Virtual(concrete_family) = output.local.functions
        [concrete_base]
        .method
        .expect("concrete base method metadata")
        .dispatch
    else {
        panic!("the concrete base method must retain its virtual family")
    };
    assert_eq!(
        output
            .local
            .dispatch_slot_identities
            .virtual_slot(concrete_family)
            .id(),
        module.dispatch_slot_identities[family].id()
    );

    let stable = virtual_fixture(true);
    let stable_base = function_named(&stable.export, "Base.value");
    let hir::MethodDispatch::Virtual(stable_family) = stable.export.functions[stable_base]
        .method
        .expect("stable base method metadata")
        .dispatch
    else {
        panic!("the stable base method must own a virtual family")
    };
    assert_eq!(
        module.dispatch_slot_identities[family].id(),
        stable.export.dispatch_slot_identities[stable_family].id()
    );
}

#[test]
fn interface_function_and_property_accessors_have_distinct_typed_slots() {
    let output = lower(vec![interface("Contract"), fun("main", Vec::new())], false);
    let module = &output.export;
    let contract = module
        .interfaces
        .iter()
        .find(|(_, declaration)| declaration.name == "Contract")
        .expect("Contract interface")
        .1;
    assert_eq!(contract.methods.len(), 3);

    let mut roles = Vec::new();
    let mut identities = Vec::new();
    for member in &contract.methods {
        let declaration = &module.interface_methods[*member];
        let slot = &module.dispatch_slot_identities[*member];
        let expected = match declaration.role {
            hir::InterfaceMemberRole::Function => DispatchSlotKey::interface_method(
                plain_function_record(module, declaration.function).id(),
            ),
            hir::InterfaceMemberRole::PropertyGetter(_) => {
                let hir::HirFunctionIdentity::PropertyAccessor(
                    hir::HirPropertyAccessorFunction::Getter(getter),
                ) = module.function_identities[declaration.function]
                else {
                    panic!("interface getter function identity")
                };
                DispatchSlotKey::property_getter(module.property_accessor_identities[getter].id())
            }
            hir::InterfaceMemberRole::PropertySetter(_) => {
                let hir::HirFunctionIdentity::PropertyAccessor(
                    hir::HirPropertyAccessorFunction::Setter(setter),
                ) = module.function_identities[declaration.function]
                else {
                    panic!("interface setter function identity")
                };
                DispatchSlotKey::property_setter(module.property_accessor_identities[setter].id())
            }
        };
        assert_eq!(slot.key(), &expected);
        roles.push(slot.key().role());
        identities.push(slot.id());
    }
    roles.sort();
    assert_eq!(
        roles,
        vec![
            DispatchRole::InterfaceMethod,
            DispatchRole::PropertyGetter,
            DispatchRole::PropertySetter,
        ]
    );
    identities.sort();
    identities.dedup();
    assert_eq!(identities.len(), 3);

    let local_contract = output
        .local
        .interfaces
        .iter()
        .find(|(_, declaration)| declaration.name == "Contract")
        .expect("concrete Contract interface")
        .0;
    let local_identities = output
        .local
        .dispatch_slot_identities
        .interface_slots(local_contract)
        .map(|(_, record)| record.id())
        .collect::<Vec<_>>();
    assert_eq!(local_identities, identities_for_contract(module, contract));
}

fn identities_for_contract(
    module: &hir::Module,
    contract: &hir::InterfaceDecl,
) -> Vec<scoop_identity::PersistentDispatchSlotId> {
    contract
        .methods
        .iter()
        .map(|member| module.dispatch_slot_identities[*member].id())
        .collect()
}

#[test]
fn dispatch_identity_relation_rejects_a_wrong_virtual_slot_role() {
    let output = virtual_fixture(false);
    let module = &output.export;
    let base = function_named(module, "Base.value");
    let hir::MethodDispatch::Virtual(target_family) = module.functions[base]
        .method
        .expect("base method metadata")
        .dispatch
    else {
        panic!("the base method must own a virtual family")
    };
    let roots = module
        .functions
        .iter()
        .filter_map(|(function, declaration)| {
            let hir::MethodDispatch::Virtual(family) = declaration.method?.dispatch else {
                return None;
            };
            Some((family, function))
        })
        .collect::<BTreeMap<_, _>>();
    let virtual_slots = roots
        .into_iter()
        .map(|(family, root)| {
            let record = if family == target_family {
                CborIdentityRecord::from_key(DispatchSlotKey::interface_method(
                    plain_function_record(module, root).id(),
                ))
                .unwrap()
            } else {
                module.dispatch_slot_identities[family].clone()
            };
            (family, root, record)
        })
        .collect();
    let interface_slots = module
        .interface_methods
        .iter()
        .map(|(member, _)| module.dispatch_slot_identities[member].clone())
        .collect();

    assert!(matches!(
        hir::HirDispatchSlotIdentities::checked(
            hir::HirDispatchSlotIdentityInputs {
                functions: &module.functions,
                function_identities: &module.function_identities,
                property_accessor_identities: &module.property_accessor_identities,
                interface_methods: &module.interface_methods,
            },
            virtual_slots,
            interface_slots,
        ),
        Err(hir::HirDispatchSlotIdentityError::VirtualMemberRole { family, .. })
            if family == target_family.into_raw()
    ));
}
