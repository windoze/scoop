use super::super::*;
use super::{
    capability_interfaces, coroutine_core_declarations, exception_core_declarations,
    ffi_core_declarations, intrinsic_type_declarations,
};

/// The minimal `scoop.core` (sysroot): the `Option<T>` enum (spec 7.2),
/// the complete compiler exception core (spec 11.7), the M10 coroutine
/// protocol, plus the M7
/// `io.scoop` final shape
/// (docs/milestone7/DESIGN.md section 2) — the managed `write` extern
/// intrinsic and `print` / `println` as ordinary `Any`-parameter
/// functions dispatching `toString()`.
pub(crate) fn core_file() -> SourceFile {
    let mut declarations = capability_interfaces();
    declarations.extend(intrinsic_type_declarations());
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
    file(declarations)
}
