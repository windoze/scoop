use super::super::*;

pub(super) fn ffi_core_declarations() -> Vec<Decl> {
    let marker = |name: &str| ast::Annotation {
        name: ident(name),
        args: Vec::new(),
        span: sp(),
    };
    let intrinsic = |name: &str| ast::Annotation {
        name: ident("Intrinsic"),
        args: vec![ast::AnnotationArg {
            name: None,
            value: ast::AnnotationLiteral::String(name.to_string()),
            span: sp(),
        }],
        span: sp(),
    };
    let pointer_method = |name: &str,
                          intrinsic_name: &str,
                          type_params: Vec<&str>,
                          params: Vec<(&str, TypeRef)>,
                          return_ty: Option<TypeRef>| {
        let mut method = method_full(false, false, name, params, return_ty, FunctionBody::None);
        method.annotations = vec![marker("NoGC"), marker("Unsafe"), intrinsic(intrinsic_name)];
        method.type_params = type_params.into_iter().map(type_param).collect();
        for param in &mut method.type_params {
            param.inline_bound = Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value));
        }
        method
    };

    let mut ptr = generic_struct_decl_full(
        "Ptr",
        vec!["T"],
        vec![("_rawPointer", ty_named("UInt"))],
        Vec::new(),
        vec![
            pointer_method(
                "toUInt",
                "ptr_to_uint",
                vec![],
                vec![],
                Some(ty_named("UInt")),
            ),
            pointer_method(
                "cast",
                "ptr_cast",
                vec!["U"],
                vec![],
                Some(ty_generic("Ptr", vec![ty_named("U")])),
            ),
            pointer_method("load", "ptr_load", vec![], vec![], Some(ty_named("T"))),
            pointer_method(
                "load",
                "ptr_load_offset",
                vec![],
                vec![("offset", ty_named("Int"))],
                Some(ty_named("T")),
            ),
            pointer_method(
                "store",
                "ptr_store",
                vec![],
                vec![("value", ty_named("T"))],
                None,
            ),
            pointer_method(
                "store",
                "ptr_store_offset",
                vec![],
                vec![("offset", ty_named("Int")), ("value", ty_named("T"))],
                None,
            ),
            pointer_method(
                "plus",
                "ptr_plus",
                vec![],
                vec![("offset", ty_named("Int"))],
                Some(ty_generic("Ptr", vec![ty_named("T")])),
            ),
            pointer_method(
                "minus",
                "ptr_minus",
                vec![],
                vec![("offset", ty_named("Int"))],
                Some(ty_generic("Ptr", vec![ty_named("T")])),
            ),
        ],
    );
    let Decl::Struct(ptr_decl) = &mut ptr else {
        unreachable!()
    };
    ptr_decl.type_params[0].inline_bound =
        Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value));

    let fun_ptr = generic_struct_decl("FunPtr", vec!["F"], vec![("_rawPointer", ty_named("UInt"))]);

    let ref_bound = |mut decl: Decl| {
        let type_params = match &mut decl {
            Decl::Struct(decl) => &mut decl.type_params,
            Decl::Function(decl) => &mut decl.type_params,
            _ => unreachable!("FFI core declarations are structs or functions"),
        };
        type_params[0].inline_bound = Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Ref));
        decl
    };
    let gc_intrinsic =
        |name: &str, intrinsic_name: &str, params: Vec<(&str, TypeRef)>, return_ty: TypeRef| {
            let mut decl =
                intrinsic_generic_fun(name, intrinsic_name, vec!["T"], params, Some(return_ty));
            let Decl::Function(function) = &mut decl else {
                unreachable!()
            };
            function.annotations = vec![marker("Unsafe"), intrinsic(intrinsic_name)];
            ref_bound(decl)
        };
    let top_level = |name: &str,
                     intrinsic_name: &str,
                     params: Vec<(&str, TypeRef)>,
                     return_ty: TypeRef,
                     no_gc: bool,
                     unsafe_: bool| {
        let mut decl =
            intrinsic_generic_fun(name, intrinsic_name, vec!["T"], params, Some(return_ty));
        let Decl::Function(function) = &mut decl else {
            unreachable!()
        };
        function.type_params[0].inline_bound =
            Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value));
        function.annotations.clear();
        if no_gc {
            function.annotations.push(marker("NoGC"));
        }
        if unsafe_ {
            function.annotations.push(marker("Unsafe"));
        }
        function.annotations.push(intrinsic(intrinsic_name));
        decl
    };

    let foreign_callback_intrinsic =
        |name: &str, intrinsic_name: &str, params: Vec<(&str, TypeRef)>, return_ty: TypeRef| {
            let mut decl =
                intrinsic_generic_fun(name, intrinsic_name, vec!["F"], params, Some(return_ty));
            let Decl::Function(function) = &mut decl else {
                unreachable!()
            };
            function.annotations = vec![marker("Unsafe"), intrinsic(intrinsic_name)];
            decl
        };

    vec![
        ptr,
        fun_ptr,
        enum_decl(
            "ForeignCallbackMode",
            vec![],
            vec![variant_unit("Reusable"), variant_unit("OneShot")],
        ),
        enum_decl(
            "ForeignCallbackState",
            vec![],
            vec![
                variant_unit("Registered"),
                variant_unit("Active"),
                variant_unit("Completed"),
                variant_unit("Failed"),
            ],
        ),
        generic_struct_decl(
            "ForeignCallback",
            vec!["F"],
            vec![
                ("function", ty_generic("FunPtr", vec![ty_named("F")])),
                ("context", ty_generic("Ptr", vec![ty_named("Unit")])),
            ],
        ),
        foreign_callback_intrinsic(
            "foreignCallback",
            "foreign_callback_register",
            vec![
                ("callback", ty_named("Any")),
                ("contextIndex", ty_named("Int")),
                ("mode", ty_named("ForeignCallbackMode")),
            ],
            ty_generic("ForeignCallback", vec![ty_named("F")]),
        ),
        foreign_callback_intrinsic(
            "retainForeignCallback",
            "foreign_callback_retain",
            vec![(
                "callback",
                ty_generic("ForeignCallback", vec![ty_named("F")]),
            )],
            ty_generic("ForeignCallback", vec![ty_named("F")]),
        ),
        foreign_callback_intrinsic(
            "releaseForeignCallback",
            "foreign_callback_release",
            vec![(
                "callback",
                ty_generic("ForeignCallback", vec![ty_named("F")]),
            )],
            ty_named("Unit"),
        ),
        foreign_callback_intrinsic(
            "foreignCallbackState",
            "foreign_callback_state",
            vec![(
                "callback",
                ty_generic("ForeignCallback", vec![ty_named("F")]),
            )],
            ty_named("ForeignCallbackState"),
        ),
        foreign_callback_intrinsic(
            "foreignCallbackFailure",
            "foreign_callback_failure",
            vec![(
                "callback",
                ty_generic("ForeignCallback", vec![ty_named("F")]),
            )],
            ty_nullable(ty_named("Throwable")),
        ),
        ref_bound(generic_struct_decl(
            "PinnedPtr",
            vec!["T"],
            vec![("raw", ty_named("UInt"))],
        )),
        ref_bound(generic_struct_decl(
            "GcHandle",
            vec!["T"],
            vec![("raw", ty_named("UInt"))],
        )),
        gc_intrinsic(
            "_pin",
            "gc_pin_raw",
            vec![("v", ty_named("T"))],
            ty_named("UInt"),
        ),
        gc_intrinsic(
            "_unpin",
            "gc_unpin_raw",
            vec![("raw", ty_named("UInt"))],
            ty_named("T"),
        ),
        gc_intrinsic(
            "_getGcHandle",
            "gc_get_handle_raw",
            vec![("v", ty_named("T"))],
            ty_named("UInt"),
        ),
        gc_intrinsic(
            "_releaseGcHandle",
            "gc_release_handle_raw",
            vec![("raw", ty_named("UInt"))],
            ty_named("T"),
        ),
        top_level(
            "addressOf",
            "address_of",
            vec![("value", ty_named("T"))],
            ty_generic("Ptr", vec![ty_named("T")]),
            false,
            true,
        ),
        top_level("sizeOf", "size_of", vec![], ty_named("UInt"), true, false),
        top_level("alignOf", "align_of", vec![], ty_named("UInt"), true, false),
    ]
}
