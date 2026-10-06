use scoop_ast::{
    AccessorBodySyntax, ClassConstructorDecl, ClassMember, CompanionNameSyntax, Decl,
    DeclaredVisibility, Expr, FunctionBody, NestedNominalDecl, PropertyBodySyntax,
    SetterParameterSyntax, SetterVisibilitySyntax, StatementKind, StructMember, VisibilitySyntax,
};

use crate::parse;
use crate::tests::{block_body, err, ok, only_function};

fn explicit_visibility(syntax: VisibilitySyntax) -> DeclaredVisibility {
    match syntax {
        VisibilitySyntax::Explicit { visibility, .. } => visibility,
        VisibilitySyntax::Omitted => panic!("expected explicit visibility"),
    }
}

#[test]
fn complete_property_and_accessor_syntax_has_one_closed_body_form() {
    let file = ok("class C {\n\
             @Property protected open override var value: Int = 1\n\
             @Getter get() = field\n\
             @Setter private set(next) { save(next) }\n\
         }\n");
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class")
    };
    let ClassMember::StoredProperty(property) = &class.members[0] else {
        panic!("expected property")
    };
    assert!(property.mutable);
    assert!(property.is_override);
    assert_eq!(property.modifier, scoop_ast::MethodModifier::Open);
    assert_eq!(
        explicit_visibility(property.visibility),
        DeclaredVisibility::Protected
    );
    let PropertyBodySyntax::Initializer {
        expression,
        accessors,
    } = &property.body
    else {
        panic!("expected initialized stored property")
    };
    assert!(matches!(
        &**expression,
        Expr::IntLiteral(literal) if literal.magnitude == 1
    ));
    let getter = accessors.getter.as_ref().expect("getter");
    assert_eq!(getter.annotations[0].name.text, "Getter");
    assert!(matches!(getter.body, AccessorBodySyntax::Expr(_)));
    let setter = accessors.setter.as_ref().expect("setter");
    assert_eq!(setter.annotations[0].name.text, "Setter");
    assert!(matches!(
        setter.visibility,
        SetterVisibilitySyntax::Explicit {
            visibility: DeclaredVisibility::Private,
            ..
        }
    ));
    assert!(matches!(
        &setter.parameter,
        SetterParameterSyntax::Named(name) if name.text == "next"
    ));
    assert!(matches!(setter.body, AccessorBodySyntax::Block(_)));
}

#[test]
fn every_property_source_form_is_preserved() {
    let file = ok("val optional: String?\n\
         val delegated: Int by makeDelegate()\n\
         const val answer: Int = 42\n\
         @Extern(\"native\") val raw: Int\n\
         val <T> Box<T>.first: T where T : ref get() = extract(this)\n");
    let properties = file
        .declarations
        .iter()
        .map(|declaration| match declaration {
            Decl::Global(property) => property,
            _ => panic!("expected global property"),
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        properties[0].body,
        PropertyBodySyntax::OptionalOmitted
    ));
    assert!(matches!(
        properties[1].body,
        PropertyBodySyntax::Delegated { .. }
    ));
    assert!(matches!(properties[2].body, PropertyBodySyntax::Const(_)));
    assert!(matches!(
        properties[3].body,
        PropertyBodySyntax::ExternStorage
    ));
    assert_eq!(properties[4].type_params.len(), 1);
    assert!(properties[4].receiver_ty.is_some());
    assert!(properties[4].where_clause.is_some());
    assert!(matches!(
        properties[4].body,
        PropertyBodySyntax::Computed(_)
    ));
}

#[test]
fn computed_properties_parse_in_value_and_interface_bodies() {
    let file = ok("struct S {\n val doubled: Int get() = 2\n }\n\
         enum E { A\n val code: Int get() = 1\n }\n\
         interface I {\n var size: Int get() set(value)\n }\n");
    let Decl::Struct(strukt) = &file.declarations[0] else {
        panic!("expected struct")
    };
    assert!(matches!(strukt.members[0], StructMember::Property(_)));
    let Decl::Enum(enumeration) = &file.declarations[1] else {
        panic!("expected enum")
    };
    assert_eq!(enumeration.properties.len(), 1);
    let Decl::Interface(interface) = &file.declarations[2] else {
        panic!("expected interface")
    };
    let PropertyBodySyntax::Computed(accessors) = &interface.properties[0].body else {
        panic!("expected computed interface property")
    };
    assert!(accessors.getter.is_some());
    assert!(accessors.setter.is_some());
}

#[test]
fn local_delegate_and_qualified_interface_super_have_dedicated_nodes() {
    let file = ok("fun f() {\n\
             val local: Int by makeDelegate()\n\
             val read = super<I>.value\n\
             super<I>.value = 1\n\
             super<I>.run<String>()\n\
         }\n");
    let statements = &block_body(only_function(&file)).statements;
    assert!(matches!(
        statements[0].kind,
        StatementKind::LocalDelegatedProperty(_)
    ));
    let StatementKind::ValDecl(read) = &statements[1].kind else {
        panic!("expected local read")
    };
    assert!(matches!(
        read.init,
        Expr::QualifiedInterfaceSuperAccess { .. }
    ));
    let StatementKind::Assign(assign) = &statements[2].kind else {
        panic!("expected assignment")
    };
    assert!(matches!(
        assign.target,
        scoop_ast::PlaceExpr::QualifiedInterfaceSuperProperty { .. }
    ));
    let StatementKind::Expr(Expr::QualifiedInterfaceSuperMethodCall { type_args, .. }) =
        &statements[3].kind
    else {
        panic!("expected qualified super method call")
    };
    assert_eq!(type_args.len(), 1);
}

#[test]
fn object_companion_nested_and_constructor_metadata_are_explicit() {
    let file = ok("public object Registry { val count: Int = 1 }\n\
         class Box<T> @Primary private constructor(private override val value: T) {\n\
             @Secondary protected constructor() : this(value = panic()) {}\n\
             class Nested<U>\n\
             companion object Factory { fun create() = Registry }\n\
         }\n");
    assert!(matches!(file.declarations[0], Decl::Object(_)));
    let Decl::Class(class) = &file.declarations[1] else {
        panic!("expected class")
    };
    let ClassConstructorDecl::Declared(primary) = &class.constructor else {
        panic!("expected primary constructor")
    };
    assert_eq!(primary.annotations[0].name.text, "Primary");
    assert_eq!(
        explicit_visibility(primary.visibility),
        DeclaredVisibility::Private
    );
    assert!(primary.parameters[0].is_override);
    assert_eq!(
        explicit_visibility(
            primary.parameters[0]
                .member_visibility
                .expect("property visibility")
        ),
        DeclaredVisibility::Private
    );
    let ClassMember::SecondaryConstructor(secondary) = &class.members[0] else {
        panic!("expected secondary constructor")
    };
    assert_eq!(secondary.annotations[0].name.text, "Secondary");
    assert_eq!(
        explicit_visibility(secondary.visibility),
        DeclaredVisibility::Protected
    );
    assert!(matches!(
        &class.members[1],
        ClassMember::Nested(declaration)
            if matches!(&**declaration, NestedNominalDecl::Class(_))
    ));
    let ClassMember::Companion(companion) = &class.members[2] else {
        panic!("expected companion")
    };
    assert!(
        matches!(companion.name, CompanionNameSyntax::Named(ref name) if name.text == "Factory")
    );
}

#[test]
fn static_nested_type_paths_and_extension_receiver_boundary_are_structural() {
    let file = ok("class Outer<T> { class Nested<U> }\n\
         fun take(value: Outer.Nested<Int>) {}\n\
         val Outer.Nested<Int>.marker: Int get() = 1\n");
    let Decl::Function(function) = &file.declarations[1] else {
        panic!("expected function")
    };
    assert!(matches!(
        &function.params[0].ty.kind,
        scoop_ast::TypeRefKind::Qualified { path, arguments }
            if path.iter().map(|segment| segment.text.as_str()).collect::<Vec<_>>()
                == ["Outer", "Nested"]
                && arguments.len() == 1
    ));
    let Decl::Global(property) = &file.declarations[2] else {
        panic!("expected extension property")
    };
    assert_eq!(property.name.text, "marker");
    assert!(matches!(
        property.receiver_ty.as_ref().map(|receiver| &receiver.kind),
        Some(scoop_ast::TypeRefKind::Qualified { path, arguments })
            if path.len() == 2 && arguments.len() == 1
    ));
}

#[test]
fn interface_default_body_is_retained() {
    let file = ok("interface I { fun answer(): Int = 42 }");
    let Decl::Interface(interface) = &file.declarations[0] else {
        panic!("expected interface")
    };
    assert!(matches!(interface.methods[0].body, FunctionBody::Expr(_)));
}

#[test]
fn rejected_m21_forms_have_specific_diagnostics() {
    let (_, lateinit) = err("lateinit var value: String");
    assert!(lateinit.contains("Option omitted-initializer shorthand"));

    let (_, primary) = err("class C private (val value: Int)");
    assert_eq!(
        primary,
        "primary constructor annotations or visibility require the `constructor` keyword"
    );

    let (_, object_constructor) = err("object O { constructor() {} }");
    assert_eq!(
        object_constructor,
        "object declarations cannot declare constructors"
    );

    let (_, inner) = err("class Outer { inner class Inner }");
    assert_eq!(
        inner,
        "`inner` declarations are not supported; nested declarations are static"
    );
}

#[test]
fn duplicate_companion_recovers_at_the_body_item_boundary() {
    let diagnostics = parse(
        "class C {\n companion object First\n companion object Second\n fun ok() {}\n }\n\
         fun main() {}\n",
    )
    .expect_err("duplicate companion must fail");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "a nominal declaration may contain at most one companion object"
    );
}

#[test]
fn builtin_spelling_can_begin_a_qualified_package_path() {
    let file = ok("fun use(value: Unit.names.Item): Unit {}\n");
    let function = only_function(&file);
    let scoop_ast::TypeRefKind::Qualified { path, arguments } = &function.params[0].ty.kind else {
        panic!("a multi-segment name is a declaration path")
    };
    assert_eq!(
        path.iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["Unit", "names", "Item"]
    );
    assert!(arguments.is_empty());
    assert!(matches!(
        function.return_ty.as_ref().unwrap().kind,
        scoop_ast::TypeRefKind::Unit
    ));
}
