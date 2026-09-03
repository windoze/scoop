use super::super::*;

pub(super) fn capability_interfaces() -> Vec<Decl> {
    vec![
        interface_decl(
            "ToString",
            vec![method_full(
                false,
                true,
                "toString",
                Vec::new(),
                Some(ty_named("String")),
                FunctionBody::None,
            )],
        ),
        interface_decl(
            "Hash",
            vec![method_full(
                false,
                true,
                "hash",
                Vec::new(),
                Some(ty_named("Int")),
                FunctionBody::None,
            )],
        ),
    ]
}
