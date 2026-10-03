use super::super::*;
use super::{
    capability_interfaces, coroutine_core_declarations, exception_core_declarations,
    ffi_core_declarations, gc_api_declarations, intrinsic_type_declarations,
};

/// The minimal `scoop.core` (sysroot): the `Option<T>` enum (spec 7.2),
/// the complete compiler exception core (spec 11.7), the M10 coroutine
/// protocol, the M22 iteration protocols, plus the M7
/// `io.scoop` final shape
/// (docs/milestone7/DESIGN.md section 2) — the managed `write` extern
/// intrinsic and `print` / `println` as ordinary `Any`-parameter
/// functions dispatching `toString()`.
pub(crate) fn core_file() -> SourceFile {
    let mut declarations = capability_interfaces();
    declarations.extend(iteration_core_declarations());
    declarations.extend(intrinsic_type_declarations());
    declarations.extend([
        struct_decl(
            "SourceLocation",
            vec![
                ("file", ty_named("String")),
                ("line", ty_named("Long")),
                ("column", ty_named("Long")),
                ("functionName", ty_named("String")),
                ("typeName", ty_named("String")),
            ],
        ),
        intrinsic_fun(
            "getCurrentSourceLocation",
            "current_source_location",
            Vec::new(),
            Some(ty_named("SourceLocation")),
        ),
    ]);
    declarations.push(enum_decl(
        "Option",
        vec!["T"],
        vec![
            variant_positional("Some", vec![ty_named("T")]),
            variant_unit("None"),
        ],
    ));
    declarations.extend(exception_core_declarations());
    declarations.extend(coroutine_core_declarations());
    declarations.extend(ffi_core_declarations());
    let mut print = fun_expr(
        "print",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        call("write", vec![method_call(var("value"), "toString", vec![])]),
    );
    let Decl::Function(print_decl) = &mut print else {
        unreachable!()
    };
    print_decl.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_named("ToString")));
    let mut println = fun_sig(
        "println",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        vec![
            stmt(call(
                "write",
                vec![method_call(var("value"), "toString", vec![])],
            )),
            stmt(call("write", vec![str_lit("\n")])),
        ],
    );
    let Decl::Function(println_decl) = &mut println else {
        unreachable!()
    };
    println_decl.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_named("ToString")));
    declarations.extend([
        scoop_extern_fun(
            "write",
            "scoop_rt_write",
            vec![("message", ty_named("String"))],
            None,
        ),
        print,
        println,
    ]);
    let mut source = file(declarations);
    make_core_public(&mut source);
    source
}

/// Complete trusted-core fixture used by the M23 production bootstrap path.
pub(crate) fn complete_core_file() -> SourceFile {
    let mut source = core_file();
    source.declarations.extend(gc_api_declarations());
    make_core_public(&mut source);
    source
}

fn iteration_core_declarations() -> Vec<Decl> {
    let mut iterator_method = bodyless_method(
        false,
        "iterator",
        Vec::new(),
        Some(ty_generic("Iterator", vec![ty_named("T")])),
    );
    iterator_method.operator = Some(ast::OperatorModifier { span: sp() });

    vec![
        generic_interface_decl(
            "Iterator",
            vec!["T"],
            vec![bodyless_method(
                false,
                "next",
                Vec::new(),
                Some(ty_generic("Option", vec![ty_named("T")])),
            )],
        ),
        generic_interface_decl("Iterable", vec!["T"], vec![iterator_method]),
    ]
}

pub(crate) fn make_core_public(source: &mut SourceFile) {
    for declaration in &mut source.declarations {
        make_declaration_public(declaration);
    }
}

fn public_visibility() -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    }
}

fn make_function_public(function: &mut ast::FunctionDecl) {
    function.visibility = public_visibility();
}

fn make_property_public(property: &mut ast::PropertyDecl) {
    property.visibility = public_visibility();
}

fn make_nested_public(declaration: &mut ast::NestedNominalDecl) {
    match declaration {
        ast::NestedNominalDecl::Struct(declaration) => make_struct_public(declaration),
        ast::NestedNominalDecl::Enum(declaration) => make_enum_public(declaration),
        ast::NestedNominalDecl::Class(declaration) => make_class_public(declaration),
        ast::NestedNominalDecl::Interface(declaration) => make_interface_public(declaration),
        ast::NestedNominalDecl::Object(declaration) => make_object_public(declaration),
    }
}

fn make_class_members_public(members: &mut [ast::ClassMember]) {
    for member in members {
        match member {
            ast::ClassMember::StoredProperty(property) => make_property_public(property),
            ast::ClassMember::InitBlock(_) | ast::ClassMember::ReleaseBlock(_) => {}
            ast::ClassMember::SecondaryConstructor(constructor) => {
                constructor.visibility = public_visibility();
            }
            ast::ClassMember::Function(function) => make_function_public(function),
            ast::ClassMember::Nested(declaration) => make_nested_public(declaration),
            ast::ClassMember::Companion(companion) => {
                companion.visibility = public_visibility();
                make_class_members_public(&mut companion.members);
            }
        }
    }
}

fn make_class_public(declaration: &mut ast::ClassDecl) {
    declaration.visibility = public_visibility();
    if let ast::ClassConstructorDecl::Declared(constructor) = &mut declaration.constructor {
        constructor.visibility = public_visibility();
        for parameter in &mut constructor.parameters {
            if parameter.property.is_property() {
                parameter.member_visibility = Some(public_visibility());
            }
        }
    }
    make_class_members_public(&mut declaration.members);
}

fn make_interface_public(declaration: &mut ast::InterfaceDecl) {
    declaration.visibility = public_visibility();
    for method in &mut declaration.methods {
        make_function_public(method);
    }
    for property in &mut declaration.properties {
        make_property_public(property);
    }
    for nested in &mut declaration.nested {
        make_nested_public(nested);
    }
    if let Some(companion) = &mut declaration.companion {
        companion.visibility = public_visibility();
        make_class_members_public(&mut companion.members);
    }
}

fn make_struct_public(declaration: &mut ast::StructDecl) {
    declaration.visibility = public_visibility();
    for member in &mut declaration.members {
        match member {
            ast::StructMember::SecondaryConstructor(constructor) => {
                constructor.visibility = public_visibility();
            }
            ast::StructMember::Function(function) => make_function_public(function),
            ast::StructMember::Property(property) => make_property_public(property),
            ast::StructMember::Nested(declaration) => make_nested_public(declaration),
            ast::StructMember::Companion(companion) => {
                companion.visibility = public_visibility();
                make_class_members_public(&mut companion.members);
            }
        }
    }
}

fn make_enum_public(declaration: &mut ast::EnumDecl) {
    declaration.visibility = public_visibility();
    for method in &mut declaration.methods {
        make_function_public(method);
    }
    for property in &mut declaration.properties {
        make_property_public(property);
    }
    for nested in &mut declaration.nested {
        make_nested_public(nested);
    }
    if let Some(companion) = &mut declaration.companion {
        companion.visibility = public_visibility();
        make_class_members_public(&mut companion.members);
    }
}

fn make_object_public(declaration: &mut ast::ObjectDecl) {
    declaration.visibility = public_visibility();
    make_class_members_public(&mut declaration.members);
}

fn make_declaration_public(declaration: &mut Decl) {
    match declaration {
        Decl::Global(property) => make_property_public(property),
        Decl::Function(function) if function.name.text != "__scoopThrowInitializationCycle" => {
            make_function_public(function)
        }
        Decl::Function(_) => {}
        Decl::TypeAlias(declaration) => declaration.visibility = public_visibility(),
        Decl::Struct(declaration) => make_struct_public(declaration),
        Decl::Enum(declaration) => make_enum_public(declaration),
        Decl::Class(declaration) => make_class_public(declaration),
        Decl::Interface(declaration) => make_interface_public(declaration),
        Decl::Object(declaration) => make_object_public(declaration),
    }
}
