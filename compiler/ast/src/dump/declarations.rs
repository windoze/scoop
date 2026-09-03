use super::super::*;
use super::{
    dump_annotations, dump_block, dump_expr, dump_type_param, dump_type_params, dump_type_ref,
    dump_where_clause,
};

pub fn dump(file: &SourceFile) -> String {
    let mut out = String::from("SourceFile\n");
    for decl in &file.declarations {
        match decl {
            Decl::Global(g) => {
                dump_annotations(&g.annotations, 2, &mut out);
                out.push_str(&format!(
                    "  {} {}: {}{}\n",
                    if g.mutable { "var" } else { "val" },
                    g.name.text,
                    dump_type_ref(&g.ty),
                    if g.init.is_some() { " = <expr>" } else { "" }
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
                    "  enum {}{}{}{}\n",
                    e.name.text, type_params, interfaces, where_clause
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
                    ClassConstructorDecl::Declared(parameters) => format!(
                        "({})",
                        parameters
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
                    "  {modifier}class {}{}{}{}{}\n",
                    c.name.text, type_params, ctor, supertypes, where_clause
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
                    "  interface {}{}{}{}\n",
                    i.name.text, params, parents, where_clause
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
                    "  struct {}{}{}{}{}\n",
                    s.name.text, type_params, representation, interfaces, where_clause
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
                    }
                }
            }
            Decl::Function(f) => {
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
                    "  {flags}fun {receiver}{}{}({}){}{}\n",
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
            out.push_str(&format!(
                "{pad}{} {}: {} =\n",
                if property.mutable { "var" } else { "val" },
                property.name.text,
                dump_type_ref(&property.ty)
            ));
            dump_expr(&property.initializer, indent + 1, out);
        }
        ClassMember::InitBlock(init) => {
            out.push_str(&format!("{pad}init\n"));
            dump_block(&init.body, indent + 1, out);
        }
        ClassMember::SecondaryConstructor(constructor) => {
            dump_secondary_constructor(constructor, indent, out)
        }
        ClassMember::Function(function) => dump_member_function(function, indent, out),
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
        "{pad}{operator}{infix}{suspend}fun {}\n",
        function.name.text
    ));
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
