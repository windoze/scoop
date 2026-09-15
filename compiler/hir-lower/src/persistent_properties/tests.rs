use scoop_ast as ast;
use scoop_hir as hir;

use super::*;
use crate::tests::{
    core_file, core_source_identity, file, fun, generic_struct_decl, ident, int_lit, sp,
    test_source_identity, ty_function, ty_generic, ty_named, ty_tuple, type_param,
};

fn package(mut source: ast::SourceFile, name: &str) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: ast::QualifiedNameSyntax {
            first: ident(name),
            rest: Vec::new(),
            span: sp(),
        },
        span: sp(),
    };
    source
}

fn property(name: &str, private: bool) -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: if private {
            ast::VisibilitySyntax::Explicit {
                visibility: ast::DeclaredVisibility::Private,
                span: sp(),
            }
        } else {
            ast::VisibilitySyntax::Omitted
        },
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(int_lit(1)),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

fn computed_property(
    name: &str,
    receiver: Option<ast::TypeRef>,
    type_parameters: &[&str],
) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: receiver,
        type_params: type_parameters
            .iter()
            .map(|name| type_param(name))
            .collect(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(int_lit(1))),
                span: sp(),
            }),
            setter: None,
        }),
        span: sp(),
    }
}

fn extension_property(name: &str, receiver: ast::TypeRef, parameters: &[&str]) -> ast::Decl {
    ast::Decl::Global(computed_property(name, Some(receiver), parameters))
}

fn object(name: &str) -> ast::Decl {
    ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    })
}

fn lower(sources: &[(u32, ast::SourceFile)], locator: &str) -> hir::Output {
    let core = core_file();
    let mut parsed = sources.iter().map(|(key, source)| {
        let path = format!("src/{key:03}.scoop");
        ast::IdentifiedParsedSource::new(test_source_identity(&path), source.clone())
    });
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        parsed.next().expect("nonempty test source set"),
        parsed.collect(),
    ))
    .expect("test source identities are unique");
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
            display_locator: locator,
            source_text: "",
        },
    )
    .expect("test source identities are valid");
    crate::lower_combined_sources(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("test current unit lowers")
}

fn ordinary_property_identities(
    output: &hir::Output,
    name: &str,
) -> Vec<scoop_identity::PersistentPropertyId> {
    let mut identities = output
        .export
        .properties
        .iter()
        .filter(|(_, property)| property.name == name)
        .map(|(id, _)| {
            output.export.property_identities[id]
                .ordinary_id()
                .expect("the selected test properties are ordinary declarations")
        })
        .collect::<Vec<_>>();
    identities.sort();
    identities
}

#[test]
fn identities_ignore_order_but_separate_package_and_private_source() {
    let mut sources = vec![
        (1, package(file(vec![property("shared", false)]), "a")),
        (2, package(file(vec![property("shared", false)]), "b")),
        (17, package(file(vec![property("secret", true)]), "same")),
        (23, package(file(vec![property("secret", true)]), "same")),
        (91, file(vec![fun("main", Vec::new())])),
    ];
    let first = lower(&sources, "/old/tree/properties.scoop");
    let first_shared = ordinary_property_identities(&first, "shared");
    let first_secret = ordinary_property_identities(&first, "secret");
    assert_eq!(first_shared.len(), 2);
    assert_eq!(first_secret.len(), 2);
    assert_ne!(first_shared[0], first_shared[1]);
    assert_ne!(first_secret[0], first_secret[1]);

    sources.reverse();
    let second = lower(&sources, "/new/tree/renamed.scoop");
    assert_eq!(
        first_shared,
        ordinary_property_identities(&second, "shared")
    );
    assert_eq!(
        first_secret,
        ordinary_property_identities(&second, "secret")
    );
}

#[test]
fn receiver_trees_use_typed_nominals_binders_structures_and_source_objects() {
    let ast::Decl::Struct(mut host) = generic_struct_decl("Host", vec!["T"], Vec::new()) else {
        unreachable!("the generic struct helper returns a struct")
    };
    host.members
        .push(ast::StructMember::Property(Box::new(computed_property(
            "member",
            None,
            &[],
        ))));

    let sources = vec![(
        1,
        file(vec![
            ast::Decl::Struct(host),
            object("Registry"),
            extension_property("unitTag", ty_named("Unit"), &[]),
            extension_property("anyTag", ty_named("Any"), &[]),
            extension_property("intTag", ty_named("Int"), &[]),
            extension_property("booleanTag", ty_named("Boolean"), &[]),
            extension_property("stringTag", ty_named("String"), &[]),
            extension_property(
                "tupleTag",
                ty_tuple(vec![ty_named("Unit"), ty_named("Any")]),
                &[],
            ),
            extension_property(
                "functionTag",
                ty_function(false, vec![ty_named("Unit")], ty_named("Any")),
                &[],
            ),
            extension_property("pointerTag", ty_generic("Ptr", vec![ty_named("Unit")]), &[]),
            extension_property(
                "functionPointerTag",
                ty_generic(
                    "FunPtr",
                    vec![ty_function(false, vec![ty_named("Int")], ty_named("Unit"))],
                ),
                &[],
            ),
            extension_property(
                "genericTag",
                ty_generic("Host", vec![ty_named("T")]),
                &["T"],
            ),
            extension_property("objectTag", ty_named("Registry"), &[]),
            fun("main", Vec::new()),
        ]),
    )];
    let output = lower(&sources, "typed-properties.scoop");

    let receiver = |name: &str| {
        let (id, _) = output
            .export
            .properties
            .iter()
            .find(|(_, property)| property.name == name)
            .expect("test extension property");
        let scoop_identity::DuplicateSignatureKey::Property {
            type_parameter_count,
            receiver: scoop_identity::OptionalSignatureType::Present(receiver),
        } = output.export.property_identities[id]
            .declaration()
            .duplicate_signature()
        else {
            panic!("extension property identity has its receiver signature")
        };
        (*type_parameter_count, receiver.as_ref().clone())
    };
    let unit = scoop_identity::SignatureTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    );
    let any = scoop_identity::SignatureTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Any
            .identity_record()
            .id(),
    );
    assert_eq!(receiver("unitTag"), (0, unit.clone()));
    assert_eq!(receiver("anyTag"), (0, any.clone()));

    let nominal = |owner: Owner| {
        match owner {
            Owner::Struct(id) => &output.export.nominal_identities[id],
            Owner::Class(id) => &output.export.nominal_identities[id],
            Owner::Interface(id) => &output.export.nominal_identities[id],
            Owner::Enum(id) => &output.export.nominal_identities[id],
            Owner::Object(id) => &output.export.nominal_identities[id],
        }
        .concrete_type_id()
        .expect("the selected intrinsic owner is concrete")
    };
    let int = scoop_identity::SignatureTypeKey::Nominal(nominal(Owner::Struct(
        output
            .export
            .core_protocols
            .fundamental_types
            .integers
            .owner(hir::IntegerKind::SIGNED_32),
    )));
    assert_eq!(receiver("intTag"), (0, int.clone()));
    assert_eq!(
        receiver("booleanTag"),
        (
            0,
            scoop_identity::SignatureTypeKey::Nominal(nominal(Owner::Struct(
                output.export.core_protocols.fundamental_types.boolean
            )))
        )
    );
    assert_eq!(
        receiver("stringTag"),
        (
            0,
            scoop_identity::SignatureTypeKey::Nominal(nominal(Owner::Class(
                output.export.core_protocols.fundamental_types.string
            )))
        )
    );
    assert_eq!(
        receiver("tupleTag"),
        (
            0,
            scoop_identity::SignatureTypeKey::Tuple(
                scoop_identity::NonEmptyVec::new(vec![unit.clone(), any.clone()]).unwrap()
            )
        )
    );
    assert_eq!(
        receiver("functionTag"),
        (
            0,
            scoop_identity::SignatureTypeKey::Function {
                effect: scoop_identity::Effect::Ordinary,
                parameters: vec![unit.clone()],
                result: Box::new(any),
            }
        )
    );
    assert_eq!(
        receiver("pointerTag"),
        (
            0,
            scoop_identity::SignatureTypeKey::RawPointer(Box::new(unit.clone()))
        )
    );
    assert_eq!(
        receiver("functionPointerTag"),
        (
            0,
            scoop_identity::SignatureTypeKey::NativeFunctionPointer {
                calling_convention: scoop_identity::CallingConvention::C,
                parameters: vec![int],
                result: Box::new(unit),
            }
        )
    );

    let (host_id, _) = output
        .export
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Host")
        .expect("Host declaration");
    assert_eq!(
        receiver("genericTag"),
        (
            1,
            scoop_identity::SignatureTypeKey::NominalApplication {
                origin: output.export.nominal_identities[host_id]
                    .generic_type_id()
                    .expect("Host is generic"),
                arguments: scoop_identity::NonEmptyVec::new(vec![
                    scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
                ])
                .unwrap(),
            }
        )
    );

    let (object_id, object) = output
        .export
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Registry")
        .expect("Registry object");
    let source_object = output.export.nominal_identities[object_id]
        .concrete_type_id()
        .expect("source objects are concrete");
    let backing = output.export.nominal_identities[object.backing_class]
        .concrete_type_id()
        .expect("object backing classes are concrete");
    assert_eq!(
        receiver("objectTag"),
        (0, scoop_identity::SignatureTypeKey::Nominal(source_object))
    );
    assert_ne!(source_object, backing);

    let (member_id, _) = output
        .export
        .properties
        .iter()
        .find(|(_, property)| property.name == "member")
        .expect("Host.member property");
    assert_eq!(
        output.export.property_identities[member_id]
            .declaration()
            .owners()
            .owners(),
        &[scoop_identity::DefinitionOwnerAtom::GenericType(
            output.export.nominal_identities[host_id]
                .generic_type_id()
                .expect("Host is generic")
        )]
    );

    let (extension_id, extension_property) = output
        .export
        .properties
        .iter()
        .find(|(_, property)| property.name == "genericTag")
        .expect("genericTag extension property");
    let getter = extension_property.capability.getter();
    let accessor = &output.export.property_accessor_identities[getter];
    assert_eq!(accessor.property(), extension_id);
    assert_eq!(
        accessor.record().key().owner(),
        output.export.property_identities[extension_id].property_owner()
    );
    assert_eq!(
        accessor.record().key().role(),
        scoop_identity::AccessorRole::Getter
    );
}
