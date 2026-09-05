use super::super::*;

pub(super) fn gc_api_declarations() -> Vec<Decl> {
    let unsafe_wrapper = |mut decl: Decl| {
        let Decl::Function(function) = &mut decl else {
            unreachable!()
        };
        function.annotations = vec![ast::Annotation {
            name: ident("Unsafe"),
            args: Vec::new(),
            span: sp(),
        }];
        function.type_params[0].inline_bound =
            Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Ref));
        decl
    };
    vec![
        unsafe_wrapper(fun_expr(
            "pin",
            vec!["T"],
            vec![("v", ty_named("T"))],
            Some(ty_generic("PinnedPtr", vec![ty_named("T")])),
            typed_call(
                "PinnedPtr",
                vec![ty_named("T")],
                vec![call("_pin", vec![var("v")])],
            ),
        )),
        unsafe_wrapper(fun_expr(
            "unpin",
            vec!["T"],
            vec![("p", ty_generic("PinnedPtr", vec![ty_named("T")]))],
            Some(ty_named("T")),
            typed_call("_unpin", vec![ty_named("T")], vec![field(var("p"), "raw")]),
        )),
        unsafe_wrapper(fun_expr(
            "getGcHandle",
            vec!["T"],
            vec![("v", ty_named("T"))],
            Some(ty_generic("GcHandle", vec![ty_named("T")])),
            typed_call(
                "GcHandle",
                vec![ty_named("T")],
                vec![call("_getGcHandle", vec![var("v")])],
            ),
        )),
        unsafe_wrapper(fun_expr(
            "releaseGcHandle",
            vec!["T"],
            vec![("h", ty_generic("GcHandle", vec![ty_named("T")]))],
            Some(ty_named("T")),
            typed_call(
                "_releaseGcHandle",
                vec![ty_named("T")],
                vec![field(var("h"), "raw")],
            ),
        )),
        intrinsic_fun("gcCollect", "rt_gc_collect", vec![], None),
        intrinsic_fun("gcStats", "rt_gc_stats", vec![], Some(ty_named("ULong"))),
    ]
}
