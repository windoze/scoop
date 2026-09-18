use scoop_identity::{DefinitionOriginSubject, SignatureTypeKey};

use super::*;

fn type_alias(name: &str, target: TypeRef, public: bool) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: if public {
            ast::VisibilitySyntax::Explicit {
                visibility: ast::DeclaredVisibility::Public,
                span: sp(),
            }
        } else {
            ast::VisibilitySyntax::Omitted
        },
        name: ident(name),
        target,
        span: sp(),
    })
}

fn lowered_alias<'module>(
    module: &'module hir::Module,
    name: &str,
) -> (hir::ExportTypeAliasId, &'module hir::TypeAliasDecl) {
    module
        .type_aliases
        .iter()
        .find(|(_, declaration)| declaration.name == name)
        .unwrap_or_else(|| panic!("missing lowered typealias `{name}`"))
}

#[test]
fn producer_projects_public_alias_edges_signatures_and_origins() {
    let module = lower_core_with_additional_declarations(vec![
        type_alias("LeafForInterface", ty_named("Int"), true),
        type_alias("DirectForInterface", ty_named("LeafForInterface"), true),
        type_alias(
            "CompositeForInterface",
            ty_tuple(vec![ty_named("LeafForInterface"), ty_named("String")]),
            true,
        ),
        type_alias("PrivateForInterface", ty_named("Int"), false),
    ]);
    let interfaces = hir::CanonicalTypeAliasInterfacesV1::from_export_hir(&module)
        .expect("the public alias surface must project canonically");
    let (leaf, leaf_declaration) = lowered_alias(&module, "LeafForInterface");
    let (direct, direct_declaration) = lowered_alias(&module, "DirectForInterface");
    let (composite, composite_declaration) = lowered_alias(&module, "CompositeForInterface");
    let (private, _) = lowered_alias(&module, "PrivateForInterface");
    let leaf_identity = module.type_alias_identities[leaf].id();
    let direct_identity = module.type_alias_identities[direct].id();
    let composite_identity = module.type_alias_identities[composite].id();
    let private_identity = module.type_alias_identities[private].id();

    let leaf_record = interfaces.get(leaf_identity).unwrap();
    let hir::TypeAliasTargetV1::Signature(leaf_signature) = leaf_record.target() else {
        panic!("a nominal alias must project as a signature")
    };
    assert_eq!(leaf_record.access(), hir::PublicLookupAccessV1::DirectOnly);
    assert_eq!(
        leaf_record.definition_origin().origin(),
        module
            .export_definition_origins
            .get(DefinitionOriginSubject::TypeAlias(leaf_identity))
            .unwrap()
            .origin()
    );

    assert_eq!(
        interfaces.get(direct_identity).unwrap().target(),
        &hir::TypeAliasTargetV1::Alias(leaf_identity)
    );
    let composite_record = interfaces.get(composite_identity).unwrap();
    let hir::TypeAliasTargetV1::Signature(SignatureTypeKey::Tuple(elements)) =
        composite_record.target()
    else {
        panic!("an alias nested in a tuple must be expanded inside one signature")
    };
    assert_eq!(&elements.as_slice()[0], leaf_signature);
    let expected_composite =
        hir::HirSignatureTypeMapper::new(hir::HirTypeIdentityInputs::from_export(&module))
            .map(composite_declaration.target, &[])
            .unwrap();
    assert_eq!(
        composite_record.target(),
        &hir::TypeAliasTargetV1::Signature(expected_composite)
    );
    assert!(interfaces.get(private_identity).is_none());
    assert_eq!(leaf_declaration.target, direct_declaration.target);
}

#[test]
fn producer_rejects_a_public_alias_edge_to_a_non_public_alias() {
    let module = lower_core_with_additional_declarations(vec![
        type_alias("HiddenAliasTarget", ty_named("Int"), false),
        type_alias("PublicAliasFacade", ty_named("HiddenAliasTarget"), true),
    ]);
    let (hidden, _) = lowered_alias(&module, "HiddenAliasTarget");
    let (facade, _) = lowered_alias(&module, "PublicAliasFacade");

    assert_eq!(
        hir::CanonicalTypeAliasInterfacesV1::from_export_hir(&module),
        Err(hir::TypeAliasInterfaceBuildError::NonPublicAliasTarget {
            alias: module.type_alias_identities[facade].id(),
            target: module.type_alias_identities[hidden].id(),
        })
    );
}
