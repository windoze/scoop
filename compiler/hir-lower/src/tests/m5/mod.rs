//! M5 tests: `Array<T>` / `MutableArray<T>` annotations, array literal
//! inference (with and without an expected type, empty literals),
//! subscript reads and writes, `.size`, and the `Array(m)` /
//! `MutableArray(a)` conversions. Golden dumps lock the output
//! structure; one negative test per diagnostic.

use super::*;

/// The `MutableArray<Int>` annotation, used by several tests.
fn ty_mutable_int_array() -> TypeRef {
    ty_generic("MutableArray", vec![ty_named("Int")])
}

fn ty_int_array() -> TypeRef {
    ty_generic("Array", vec![ty_named("Int")])
}

mod basics;
mod conversion_errors;
mod literal_errors;
mod structural;
mod subscript_errors;
mod type_errors;
