//! Class, interface, boxing and reference-operation lowering.

use super::*;

/// A `this`-taking method with an empty body, as hir-lower
/// produces it for `fun m() {}`-style declarations; the name is
/// qualified `Owner.method` like hir-lower qualifies members.
fn empty_method(
    h: &mut Harness,
    owner: &str,
    name: &str,
    receiver: hir::TypeId,
) -> hir::FunctionId {
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", receiver));
    let unit = h.unit;
    h.method_fn(
        &format!("{owner}.{name}"),
        receiver,
        vec![param("this", receiver, this)],
        unit,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    )
}

mod boxing;
mod casts;
mod construction;
mod dispatch;
mod interfaces;
mod layouts;
mod smart_casts;
