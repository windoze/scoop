//! M3 tests: function signatures, generic functions with call-site
//! type-argument inference, `Option<T>` with `Some` / `None` / `?.` /
//! `?:` / `!!` — golden dumps for inference and desugaring, one
//! negative test per diagnostic.

mod diagnostics;
mod generics;
mod option;
mod signatures;
