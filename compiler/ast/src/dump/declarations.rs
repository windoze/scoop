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
                                let default = field
                                    .default
                                    .as_ref()
                                    .map(|_| " = <expr>")
                                    .unwrap_or_default();
                                out.push_str(&format!(
                                    "      {}: {}{}\n",
                                    field.name.text,
                                    dump_type_ref(&field.ty),
                                    default
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
                    ClassConstructorDecl::Omitted => "()".to_string(),
                    ClassConstructorDecl::Declared(properties) => format!(
                        "({})",
                        properties
                            .iter()
                            .map(|p| format!(
                                "{}{}: {}",
                                if p.mutable { "var " } else { "val " },
                                p.name.text,
                                dump_type_ref(&p.ty)
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                };
                let base = c
                    .base_class
                    .as_ref()
                    .map(|(ty, args)| format!(" : {}(<{} args>)", dump_type_ref(ty), args.len()))
                    .unwrap_or_default();
                let ifaces = if c.interfaces.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = c.interfaces.iter().map(dump_type_ref).collect();
                    format!(", {}", names.join(", "))
                };
                let where_clause = dump_where_clause(c.where_clause.as_ref());
                out.push_str(&format!(
                    "  {modifier}class {}{}{}{}{}{}\n",
                    c.name.text, type_params, ctor, base, ifaces, where_clause
                ));
                for method in &c.methods {
                    let suspend = if method.is_suspend { "suspend " } else { "" };
                    let operator = if method.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "    {operator}{suspend}fun {}\n",
                        method.name.text
                    ));
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
                let parents = if i.parents.is_empty() {
                    String::new()
                } else {
                    format!(
                        " : {}",
                        i.parents
                            .iter()
                            .map(dump_type_ref)
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
                    out.push_str(&format!(
                        "    {operator}{suspend}fun {}\n",
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
                let interfaces = if s.interfaces.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = s.interfaces.iter().map(dump_type_ref).collect();
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
                        "    field {}: {}\n",
                        field.name.text,
                        dump_type_ref(&field.ty)
                    ));
                }
                for method in &s.methods {
                    let suspend = if method.is_suspend { "suspend " } else { "" };
                    let operator = if method.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "    {operator}{suspend}fun {}\n",
                        method.name.text
                    ));
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
                    .map(|p| format!("{}: {}", p.name.text, dump_type_ref(&p.ty)))
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
                    "{}{}{}{}",
                    modifier,
                    if f.is_override { "override " } else { "" },
                    if f.operator.is_some() {
                        "operator "
                    } else {
                        ""
                    },
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
