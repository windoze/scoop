use std::collections::BTreeMap;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::PersistentFieldId;

use crate::tests::{
    class_decl, core_file, core_source_identity, file, ident, int_lit, sp, struct_decl,
    test_source_identity, ty_named,
};

fn stored_property(name: &str) -> ast::PropertyDecl {
    ast::PropertyDecl {
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
            expression: Box::new(int_lit(1)),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    }
}

fn object(name: &str, property: &str) -> ast::Decl {
    ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: vec![ast::ClassMember::StoredProperty(stored_property(property))],
        span: sp(),
    })
}

fn identities(
    path: &str,
    display_locator: &str,
    struct_fields: Vec<(&str, ast::TypeRef)>,
) -> BTreeMap<String, PersistentFieldId> {
    let source = file(vec![
        struct_decl("Point", struct_fields),
        class_decl(
            ast::ClassModifier::Final,
            "Box",
            vec![(false, "value", ty_named("Int"))],
            None,
            Vec::new(),
            Vec::new(),
        ),
        object("Registry", "value"),
    ]);
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(test_source_identity(path), source),
        Vec::new(),
    ))
    .unwrap();
    let core = core_file();
    let input = crate::DefinedTestSources::try_new(
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
            display_locator,
            source_text: "",
        },
    )
    .unwrap();
    let output = crate::lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("the field-identity fixture lowers");
    let mut result = BTreeMap::new();

    let (structure_id, structure) = output
        .export
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Point")
        .unwrap();
    for (index, field) in structure.semantic_fields().iter().enumerate() {
        let field_ref = hir::StructFieldRef::checked(
            &output.export.structs,
            structure_id,
            u32::try_from(index).unwrap(),
        )
        .unwrap();
        let applied = hir::AppliedStructFieldRef::checked(
            &output.export.structs,
            &output.export.struct_applications,
            structure.self_application,
            u32::try_from(index).unwrap(),
        )
        .unwrap();
        assert_eq!(
            output.export.field_identities[field_ref].id(),
            output.export.field_identities[applied].id()
        );
        result.insert(
            format!("Point.{}", field.name),
            output.export.field_identities[field_ref].id(),
        );
    }

    let (_, class) = output
        .export
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name == "Box")
        .unwrap();
    let class_field = class.fields[0];
    result.insert(
        "Box.value".to_string(),
        output.export.field_identities[class_field].id(),
    );

    let (_, object) = output
        .export
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Registry")
        .unwrap();
    let object_field = output.export.classes[object.backing_class].fields[0];
    result.insert(
        "Registry.value".to_string(),
        output.export.field_identities[object_field].id(),
    );
    result
}

#[test]
fn field_identities_ignore_order_types_and_display_locators() {
    let first = identities(
        "src/first.scoop",
        "/old/tree/first.scoop",
        vec![("x", ty_named("Int")), ("y", ty_named("String"))],
    );
    let moved = identities(
        "src/moved.scoop",
        "/new/tree/moved.scoop",
        vec![("y", ty_named("Int")), ("x", ty_named("String"))],
    );

    assert_eq!(first, moved);
    assert_ne!(first["Box.value"], first["Registry.value"]);
}
