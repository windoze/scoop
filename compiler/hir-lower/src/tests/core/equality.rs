use super::super::*;

pub(super) fn adopt_equality(declarations: &mut [Decl]) {
    for declaration in declarations {
        match declaration {
            Decl::Struct(value) if comparable(&value.name.text) => {
                let owner = if value.type_params.is_empty() {
                    ty_named(&value.name.text)
                } else {
                    ty_generic(
                        &value.name.text,
                        value
                            .type_params
                            .iter()
                            .map(|p| ty_named(&p.name.text))
                            .collect(),
                    )
                };
                value.supertypes.push(supertype(owner.clone()));
                if value.name.text == "Ptr" {
                    let body =
                        FunctionBody::Block(block(vec![unsafe_block(vec![ret(Some(binary(
                            BinOp::Eq,
                            method_call(this_expr(), "toULong", vec![]),
                            method_call(var("other"), "toULong", vec![]),
                        )))])]));
                    value
                        .members
                        .push(ast::StructMember::Function(Box::new(method_full(
                            true,
                            false,
                            "equals",
                            vec![("other", owner.clone())],
                            Some(ty_named("Boolean")),
                            body,
                        ))));
                }
                for member in &mut value.members {
                    if let ast::StructMember::Function(function) = member
                        && function.name.text == "equals"
                    {
                        function.is_override = false;
                        function.modifier = ast::MethodModifier::Final;
                        function.operator = Some(ast::OperatorModifier { span: sp() });
                    }
                }
                if !value.members.iter().any(|member| {
                    matches!(member,
                    ast::StructMember::Function(function) if function.name.text == "equalTo")
                }) {
                    let no_gc = value.members.iter().any(|member| matches!(member,
                        ast::StructMember::Function(function) if function.name.text == "equals"
                            && function.annotations.iter().any(|annotation| annotation.name.text == "NoGC")));
                    let mut method = equal_to(owner, no_gc);
                    if value.name.text == "Unit" {
                        method.body = FunctionBody::Expr(Box::new(bool_lit(true)));
                    }
                    value
                        .members
                        .push(ast::StructMember::Function(Box::new(method)));
                }
            }
            Decl::Class(value) if value.name.text == "String" => {
                value.supertypes.push(supertype(ty_named("String")));
                for member in &mut value.members {
                    if let ast::ClassMember::Function(function) = member
                        && function.name.text == "equals"
                    {
                        function.is_override = false;
                        function.modifier = ast::MethodModifier::Final;
                    }
                }
                value.members.push(ast::ClassMember::Function(equal_to(
                    ty_named("String"),
                    false,
                )));
            }
            _ => {}
        }
    }
}

fn equal_to(owner: TypeRef, no_gc: bool) -> ast::FunctionDecl {
    let mut method = method_full(
        true,
        false,
        "equalTo",
        vec![("other", owner)],
        Some(ty_named("Boolean")),
        FunctionBody::Expr(Box::new(method_call(
            this_expr(),
            "equals",
            vec![var("other")],
        ))),
    );
    if no_gc {
        method.annotations.push(ast::Annotation {
            name: ident("NoGC"),
            args: Vec::new(),
            span: sp(),
        });
    }
    method
}

fn supertype(owner: TypeRef) -> ast::SupertypeSpec {
    ast::SupertypeSpec {
        ty: ty_generic("Equality", vec![owner]),
        constructor_arguments: None,
        span: sp(),
    }
}

fn comparable(name: &str) -> bool {
    matches!(
        name,
        "Unit"
            | "Ptr"
            | "Boolean"
            | "Char"
            | "Float"
            | "Double"
            | "Int8"
            | "Int16"
            | "Int"
            | "Long"
            | "UInt8"
            | "UInt16"
            | "UInt"
            | "ULong"
    )
}
