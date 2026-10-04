use super::super::*;
use super::{
    dump_annotations, dump_block, dump_expr, dump_headers, dump_type_param, dump_type_params,
    dump_type_ref, dump_where_clause,
};

pub fn dump(file: &SourceFile) -> String {
    let mut out = String::from("SourceFile\n");
    dump_headers(file, &mut out);
    for decl in &file.declarations {
        match decl {
            Decl::Global(g) => {
                dump_global_property(g, &mut out);
            }
            Decl::TypeAlias(alias) => {
                out.push_str(&format!(
                    "  {}typealias {} = {}\n",
                    dump_visibility(alias.visibility),
                    alias.name.text,
                    dump_type_ref(&alias.target)
                ));
            }
            Decl::Enum(e) => {
                dump_annotations(&e.annotations, 2, &mut out);
                let type_params = if e.type_params.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        e.type_params
                            .iter()
                            .map(dump_type_param)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let interfaces = if e.interfaces.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = e.interfaces.iter().map(dump_type_ref).collect();
                    format!(" : {}", names.join(", "))
                };
                let where_clause = dump_where_clause(e.where_clause.as_ref());
                out.push_str(&format!(
                    "  {}enum {}{}{}{}\n",
                    dump_visibility(e.visibility),
                    e.name.text,
                    type_params,
                    interfaces,
                    where_clause
                ));
                for variant in &e.variants {
                    match &variant.kind {
                        VariantDeclKind::Unit => {
                            out.push_str(&format!("    {}\n", variant.name.text))
                        }
                        VariantDeclKind::Positional(types) => {
                            let types: Vec<String> = types.iter().map(dump_type_ref).collect();
                            out.push_str(&format!(
                                "    {}({})\n",
                                variant.name.text,
                                types.join(", ")
                            ));
                        }
                        VariantDeclKind::Named(fields) | VariantDeclKind::Constructor(fields) => {
                            let kind = if matches!(variant.kind, VariantDeclKind::Constructor(_)) {
                                "ctor"
                            } else {
                                "named"
                            };
                            out.push_str(&format!("    {} <{}>\n", variant.name.text, kind));
                            for field in fields {
                                out.push_str(&format!(
                                    "      {}\n",
                                    dump_parameter(&field.name.text, &field.ty, &field.syntax)
                                ));
                            }
                        }
                    }
                }
                for property in &e.properties {
                    dump_property(property, 2, &mut out);
                }
                for declaration in &e.nested {
                    dump_nested_nominal(declaration, 2, &mut out);
                }
                if let Some(companion) = &e.companion {
                    dump_companion(companion, 2, &mut out);
                }
            }
            Decl::Class(c) => {
                dump_annotations(&c.annotations, 2, &mut out);
                let modifier = match c.modifier {
                    ClassModifier::Final => "",
                    ClassModifier::Open => "open ",
                    ClassModifier::Abstract => "abstract ",
                };
                let type_params = dump_type_params(&c.type_params);
                let ctor = match &c.constructor {
                    ClassConstructorDecl::Omitted => "()".to_owned(),
                    ClassConstructorDecl::Declared(constructor) => format!(
                        "({})",
                        constructor
                            .parameters
                            .iter()
                            .map(|p| format!(
                                "{}{}",
                                match p.property {
                                    PrimaryParameterProperty::Plain => "",
                                    PrimaryParameterProperty::Val => "val ",
                                    PrimaryParameterProperty::Var => "var ",
                                },
                                dump_parameter(&p.name.text, &p.ty, &p.syntax)
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                };
                let supertypes = if c.supertypes.is_empty() {
                    String::new()
                } else {
                    let separator = if c
                        .supertypes
                        .iter()
                        .any(|supertype| supertype.constructor_arguments.is_some())
                    {
                        " : "
                    } else {
                        ", "
                    };
                    format!(
                        "{separator}{}",
                        c.supertypes
                            .iter()
                            .map(dump_supertype)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let where_clause = dump_where_clause(c.where_clause.as_ref());
                out.push_str(&format!(
                    "  {}{modifier}class {}{}{}{}{}\n",
                    dump_visibility(c.visibility),
                    c.name.text,
                    type_params,
                    ctor,
                    supertypes,
                    where_clause
                ));
                for member in &c.members {
                    dump_class_member(member, 2, &mut out);
                }
            }
            Decl::Interface(i) => {
                dump_annotations(&i.annotations, 2, &mut out);
                let params = if i.type_params.is_empty() {
                    String::new()
                } else {
                    let params: Vec<String> = i.type_params.iter().map(dump_type_param).collect();
                    format!("<{}>", params.join(", "))
                };
                let parents = if i.supertypes.is_empty() {
                    String::new()
                } else {
                    format!(
                        " : {}",
                        i.supertypes
                            .iter()
                            .map(dump_supertype)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let where_clause = dump_where_clause(i.where_clause.as_ref());
                out.push_str(&format!(
                    "  {}interface {}{}{}{}\n",
                    dump_visibility(i.visibility),
                    i.name.text,
                    params,
                    parents,
                    where_clause
                ));
                for method in &i.methods {
                    let suspend = if method.is_suspend { "suspend " } else { "" };
                    let operator = if method.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    };
                    let infix = if method.infix.is_some() { "infix " } else { "" };
                    out.push_str(&format!(
                        "    {operator}{infix}{suspend}fun {}\n",
                        method.name.text
                    ));
                }
                for property in &i.properties {
                    dump_property(property, 2, &mut out);
                }
                for declaration in &i.nested {
                    dump_nested_nominal(declaration, 2, &mut out);
                }
                if let Some(companion) = &i.companion {
                    dump_companion(companion, 2, &mut out);
                }
            }
            Decl::Struct(s) => {
                dump_annotations(&s.annotations, 2, &mut out);
                let type_params = if s.type_params.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        s.type_params
                            .iter()
                            .map(dump_type_param)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let interfaces = if s.supertypes.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = s.supertypes.iter().map(dump_supertype).collect();
                    format!(" : {}", names.join(", "))
                };
                let representation = if s.fields.is_omitted() {
                    " <representation omitted>"
                } else {
                    ""
                };
                let where_clause = dump_where_clause(s.where_clause.as_ref());
                out.push_str(&format!(
                    "  {}struct {}{}{}{}{}\n",
                    dump_visibility(s.visibility),
                    s.name.text,
                    type_params,
                    representation,
                    interfaces,
                    where_clause
                ));
                for field in &s.fields {
                    out.push_str(&format!(
                        "    field {}\n",
                        dump_parameter(&field.name.text, &field.ty, &field.syntax)
                    ));
                }
                for member in &s.members {
                    match member {
                        StructMember::SecondaryConstructor(constructor) => {
                            dump_secondary_constructor(constructor, 2, &mut out)
                        }
                        StructMember::Function(method) => dump_member_function(method, 2, &mut out),
                        StructMember::Property(property) => dump_property(property, 2, &mut out),
                        StructMember::Nested(declaration) => {
                            dump_nested_nominal(declaration, 2, &mut out)
                        }
                        StructMember::Companion(companion) => {
                            dump_companion(companion, 2, &mut out)
                        }
                    }
                }
            }
            Decl::Object(object) => {
                dump_annotations(&object.annotations, 2, &mut out);
                out.push_str(&format!(
                    "  {}object {}\n",
                    dump_visibility(object.visibility),
                    object.name.text
                ));
                for member in &object.members {
                    dump_class_member(member, 2, &mut out);
                }
            }
            Decl::Function(f) => {
                super::dump_context(&f.context_parameters, 2, &mut out);
                dump_annotations(&f.annotations, 2, &mut out);
                let type_params = if f.type_params.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<{}>",
                        f.type_params
                            .iter()
                            .map(dump_type_param)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| dump_parameter(&p.name.text, &p.ty, &p.syntax))
                    .collect();
                let ret = f
                    .return_ty
                    .as_ref()
                    .map(|t| format!(": {}", dump_type_ref(t)))
                    .unwrap_or_default();
                let modifier = match (f.modifier, f.is_override) {
                    (MethodModifier::Final, false) => "",
                    (MethodModifier::Final, true) => "final ",
                    (MethodModifier::Open, false) => "open ",
                    // `override` is open by default, so avoid printing
                    // a redundant effective-modality marker.
                    (MethodModifier::Open, true) => "",
                    (MethodModifier::Abstract, _) => "abstract ",
                };
                let flags = format!(
                    "{}{}{}{}{}",
                    modifier,
                    if f.is_override { "override " } else { "" },
                    if f.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    },
                    if f.infix.is_some() { "infix " } else { "" },
                    if f.is_suspend { "suspend " } else { "" }
                );
                let receiver = f
                    .receiver_ty
                    .as_ref()
                    .map(|ty| format!("{}.", dump_type_ref(ty)))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  {}{flags}fun {receiver}{}{}({}){}{}\n",
                    dump_visibility(f.visibility),
                    f.name.text,
                    type_params,
                    params.join(", "),
                    ret,
                    dump_where_clause(f.where_clause.as_ref())
                ));
                match &f.body {
                    FunctionBody::Block(block) => dump_block(block, 2, &mut out),
                    FunctionBody::Expr(expr) => {
                        out.push_str(
                            "    =
",
                        );
                        dump_expr(expr, 3, &mut out);
                    }
                    FunctionBody::None => {}
                }
            }
        }
    }
    out
}

fn dump_supertype(supertype: &SupertypeSpec) -> String {
    let ty = dump_type_ref(&supertype.ty);
    match &supertype.constructor_arguments {
        Some(arguments) => format!("{ty}(<{} args>)", arguments.len()),
        None => ty,
    }
}

fn dump_class_member(member: &ClassMember, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match member {
        ClassMember::StoredProperty(property) => {
            if let PropertyBodySyntax::Initializer {
                expression,
                accessors,
            } = &property.body
                && property.annotations.is_empty()
                && matches!(property.visibility, VisibilitySyntax::Omitted)
                && property.modifier == MethodModifier::Final
                && !property.is_override
                && property.receiver_ty.is_none()
                && property.type_params.is_empty()
                && accessors.getter.is_none()
                && accessors.setter.is_none()
            {
                out.push_str(&format!(
                    "{pad}{} {}: {} =\n",
                    if property.mutable { "var" } else { "val" },
                    property.name.text,
                    dump_type_ref(&property.ty)
                ));
                dump_expr(expression, indent + 1, out);
            } else {
                dump_property(property, indent, out);
            }
        }
        ClassMember::InitBlock(init) => {
            out.push_str(&format!("{pad}init\n"));
            dump_block(&init.body, indent + 1, out);
        }
        ClassMember::ReleaseBlock(release) => {
            out.push_str(&format!("{pad}release\n"));
            dump_block(&release.body, indent + 1, out);
        }
        ClassMember::SecondaryConstructor(constructor) => {
            dump_secondary_constructor(constructor, indent, out)
        }
        ClassMember::Function(function) => dump_member_function(function, indent, out),
        ClassMember::Nested(declaration) => dump_nested_nominal(declaration, indent, out),
        ClassMember::Companion(companion) => dump_companion(companion, indent, out),
    }
}

fn dump_secondary_constructor(
    constructor: &SecondaryConstructorDecl,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    let params = constructor
        .params
        .iter()
        .map(|parameter| dump_parameter(&parameter.name.text, &parameter.ty, &parameter.syntax))
        .collect::<Vec<_>>()
        .join(", ");
    let delegation = match &constructor.delegation {
        Some(ConstructorDelegation::This { arguments, .. }) => {
            format!(" : this(<{} args>)", arguments.len())
        }
        Some(ConstructorDelegation::Super { arguments, .. }) => {
            format!(" : super(<{} args>)", arguments.len())
        }
        None => String::new(),
    };
    out.push_str(&format!("{pad}constructor({params}){delegation}\n"));
    dump_block(&constructor.body, indent + 1, out);
}

fn dump_member_function(function: &FunctionDecl, indent: usize, out: &mut String) {
    super::dump_context(&function.context_parameters, indent, out);
    let pad = "  ".repeat(indent);
    let suspend = if function.is_suspend { "suspend " } else { "" };
    let operator = if function.operator.is_some() {
        "operator "
    } else {
        ""
    };
    let infix = if function.infix.is_some() {
        "infix "
    } else {
        ""
    };
    out.push_str(&format!(
        "{pad}{}{operator}{infix}{suspend}fun {}\n",
        dump_visibility(function.visibility),
        function.name.text
    ));
}

fn dump_nested_nominal(declaration: &NestedNominalDecl, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match declaration {
        NestedNominalDecl::Struct(declaration) => out.push_str(&format!(
            "{pad}{}nested struct {}\n",
            dump_visibility(declaration.visibility),
            declaration.name.text
        )),
        NestedNominalDecl::Enum(declaration) => out.push_str(&format!(
            "{pad}{}nested enum {}\n",
            dump_visibility(declaration.visibility),
            declaration.name.text
        )),
        NestedNominalDecl::Class(declaration) => out.push_str(&format!(
            "{pad}{}nested class {}\n",
            dump_visibility(declaration.visibility),
            declaration.name.text
        )),
        NestedNominalDecl::Interface(declaration) => out.push_str(&format!(
            "{pad}{}nested interface {}\n",
            dump_visibility(declaration.visibility),
            declaration.name.text
        )),
        NestedNominalDecl::Object(declaration) => out.push_str(&format!(
            "{pad}{}nested object {}\n",
            dump_visibility(declaration.visibility),
            declaration.name.text
        )),
    }
}

fn dump_companion(companion: &CompanionObjectDecl, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let name = match &companion.name {
        CompanionNameSyntax::Default { .. } => String::new(),
        CompanionNameSyntax::Named(name) => format!(" {}", name.text),
    };
    out.push_str(&format!(
        "{pad}{}companion object{name}\n",
        dump_visibility(companion.visibility)
    ));
    for member in &companion.members {
        dump_class_member(member, indent + 1, out);
    }
}

fn dump_property(property: &PropertyDecl, indent: usize, out: &mut String) {
    super::dump_context(&property.context_parameters, indent, out);
    let pad = "  ".repeat(indent);
    dump_annotations(&property.annotations, indent, out);
    let modality = match (property.modifier, property.is_override) {
        (MethodModifier::Final, false) => "",
        (MethodModifier::Final, true) => "final ",
        (MethodModifier::Open, _) => "open ",
        (MethodModifier::Abstract, _) => "abstract ",
    };
    let override_ = if property.is_override {
        "override "
    } else {
        ""
    };
    let const_ = if matches!(property.body, PropertyBodySyntax::Const(_)) {
        "const "
    } else {
        ""
    };
    let type_params = dump_type_params(&property.type_params);
    let receiver = property
        .receiver_ty
        .as_ref()
        .map(|ty| format!("{}.", dump_type_ref(ty)))
        .unwrap_or_default();
    out.push_str(&format!(
        "{pad}{}{modality}{override_}{const_}{} {type_params}{receiver}{}: {}{}{}\n",
        dump_visibility(property.visibility),
        if property.mutable { "var" } else { "val" },
        property.name.text,
        dump_type_ref(&property.ty),
        dump_where_clause(property.where_clause.as_ref()),
        dump_property_body_suffix(&property.body)
    ));
    match &property.body {
        PropertyBodySyntax::Initializer {
            expression,
            accessors,
        } => {
            dump_expr(expression, indent + 1, out);
            dump_accessors(accessors, indent + 1, out);
        }
        PropertyBodySyntax::Delegated { expression, .. }
        | PropertyBodySyntax::Const(expression) => dump_expr(expression, indent + 1, out),
        PropertyBodySyntax::Computed(accessors) => dump_accessors(accessors, indent + 1, out),
        PropertyBodySyntax::OptionalOmitted
        | PropertyBodySyntax::Abstract
        | PropertyBodySyntax::ExternStorage => {}
    }
}

fn dump_global_property(property: &PropertyDecl, out: &mut String) {
    super::dump_context(&property.context_parameters, 2, out);
    dump_annotations(&property.annotations, 2, out);
    let type_params = dump_type_params(&property.type_params);
    let receiver = property
        .receiver_ty
        .as_ref()
        .map(|ty| format!("{}.", dump_type_ref(ty)))
        .unwrap_or_default();
    let body = match &property.body {
        PropertyBodySyntax::Initializer { .. } => " = <expr>",
        PropertyBodySyntax::OptionalOmitted | PropertyBodySyntax::ExternStorage => "",
        PropertyBodySyntax::Computed(_) => " <computed>",
        PropertyBodySyntax::Delegated { .. } => " by <expr>",
        PropertyBodySyntax::Abstract => " <abstract>",
        PropertyBodySyntax::Const(_) => " <const> = <expr>",
    };
    out.push_str(&format!(
        "  {}{} {type_params}{receiver}{}: {}{}{}\n",
        dump_visibility(property.visibility),
        if property.mutable { "var" } else { "val" },
        property.name.text,
        dump_type_ref(&property.ty),
        dump_where_clause(property.where_clause.as_ref()),
        body
    ));
}

fn dump_accessors(accessors: &AccessorSyntax, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    if let Some(getter) = &accessors.getter {
        dump_annotations(&getter.annotations, indent, out);
        out.push_str(&format!("{pad}get()\n"));
        dump_accessor_body(&getter.body, indent + 1, out);
    }
    if let Some(setter) = &accessors.setter {
        dump_annotations(&setter.annotations, indent, out);
        let visibility = match setter.visibility {
            SetterVisibilitySyntax::Explicit { visibility, span } => {
                dump_visibility(VisibilitySyntax::Explicit { visibility, span })
            }
            SetterVisibilitySyntax::Inherited => "",
        };
        let parameter = match &setter.parameter {
            SetterParameterSyntax::Default { .. } => "value",
            SetterParameterSyntax::Named(name) => &name.text,
        };
        out.push_str(&format!("{pad}{visibility}set({parameter})\n"));
        dump_accessor_body(&setter.body, indent + 1, out);
    }
}

fn dump_accessor_body(body: &AccessorBodySyntax, indent: usize, out: &mut String) {
    match body {
        AccessorBodySyntax::Block(block) => dump_block(block, indent, out),
        AccessorBodySyntax::Expr(expression) => dump_expr(expression, indent, out),
        AccessorBodySyntax::Omitted => {}
    }
}

fn dump_property_body_suffix(body: &PropertyBodySyntax) -> &'static str {
    match body {
        PropertyBodySyntax::Initializer { .. } => " = <expr>",
        PropertyBodySyntax::OptionalOmitted => " <optional omitted>",
        PropertyBodySyntax::Computed(_) => " <computed>",
        PropertyBodySyntax::Delegated { .. } => " by <expr>",
        PropertyBodySyntax::Abstract => " <abstract>",
        PropertyBodySyntax::ExternStorage => " <extern>",
        PropertyBodySyntax::Const(_) => " <const> = <expr>",
    }
}

fn dump_visibility(visibility: VisibilitySyntax) -> &'static str {
    match visibility {
        VisibilitySyntax::Explicit {
            visibility: DeclaredVisibility::Public,
            ..
        } => "public ",
        VisibilitySyntax::Explicit {
            visibility: DeclaredVisibility::Internal,
            ..
        } => "internal ",
        VisibilitySyntax::Explicit {
            visibility: DeclaredVisibility::Private,
            ..
        } => "private ",
        VisibilitySyntax::Explicit {
            visibility: DeclaredVisibility::Protected,
            ..
        } => "protected ",
        VisibilitySyntax::Omitted => "",
    }
}

fn dump_parameter(name: &str, ty: &TypeRef, syntax: &ParameterSyntax) -> String {
    let ty = dump_type_ref(ty);
    match syntax {
        ParameterSyntax::Required => format!("{name}: {ty}"),
        ParameterSyntax::Default { .. } => format!("{name}: {ty} = <expr>"),
        ParameterSyntax::Vararg {
            default: VarargDefaultSyntax::EmptyWhenOmitted,
            ..
        } => format!("vararg {name}: {ty}"),
        ParameterSyntax::Vararg {
            default: VarargDefaultSyntax::Expression { .. },
            ..
        } => format!("vararg {name}: {ty} = <expr>"),
    }
}
