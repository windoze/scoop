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
                if matches!(value.name.text.as_str(), "Unit" | "Ptr") {
                    let body = if value.name.text == "Unit" {
                        FunctionBody::Expr(Box::new(bool_lit(true)))
                    } else {
                        FunctionBody::Block(block(vec![unsafe_block(vec![ret(Some(binary(
                            BinOp::Eq,
                            method_call(this_expr(), "toULong", vec![]),
                            method_call(var("other"), "toULong", vec![]),
                        )))])]))
                    };
                    value
                        .members
                        .push(ast::StructMember::Function(Box::new(method_full(
                            true,
                            false,
                            "equals",
                            vec![("other", owner)],
                            Some(ty_named("Boolean")),
                            body,
                        ))));
                }
                for member in &mut value.members {
                    if let ast::StructMember::Function(function) = member
                        && function.name.text == "equals"
                    {
                        function.is_override = true;
                        function.operator = Some(ast::OperatorModifier { span: sp() });
                    }
                }
            }
            Decl::Class(value) if value.name.text == "String" => {
                value.supertypes.push(supertype(ty_named("String")));
                for member in &mut value.members {
                    if let ast::ClassMember::Function(function) = member
                        && function.name.text == "equals"
                    {
                        function.is_override = true;
                    }
                }
            }
            _ => {}
        }
    }
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
