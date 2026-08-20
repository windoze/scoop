//! M4 tests: enum declarations (four variant forms), variant
//! construction, `when` with guards and exhaustiveness, destructuring
//! declarations, intrinsics and the sysroot (multi-file) frame. Golden
//! dumps lock the enum/when/destructuring output structure; one
//! negative test per diagnostic.

use super::*;

fn color_decl() -> Decl {
    enum_decl(
        "Color",
        vec![],
        vec![
            variant_unit("Red"),
            variant_unit("Green"),
            variant_unit("Blue"),
        ],
    )
}

/// `enum Shape { Circle(Int), Named { w: Int, h: Int }, WithDefault(val d: Int = 0) }`.
fn shape_decl() -> Decl {
    enum_decl(
        "Shape",
        vec![],
        vec![
            variant_positional("Circle", vec![ty_named("Int")]),
            variant_named(
                "Named",
                vec![("w", ty_named("Int")), ("h", ty_named("Int"))],
            ),
            variant_constructor(
                "WithDefault",
                vec![("d", ty_named("Int"), Some(int_lit(0)))],
            ),
        ],
    )
}

mod core_contract;
mod destructuring;
mod enum_declarations;
mod intrinsic_gc_contract;
mod intrinsics;
mod variant_construction;
mod when;
