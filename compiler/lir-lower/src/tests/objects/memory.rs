//! Class construction, field memory access, and trap termination.

use super::*;

#[test]
fn class_field_reads_are_heap_loads() {
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[("a", mir::Type::Int), ("s", mir::Type::String)],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let s = locals.alloc(local("s", mir::Type::String));
    let main = b.main(
        locals,
        vec![val_decl(
            s,
            expr(
                mir::Type::String,
                mir::ExprKind::FieldAccess {
                    receiver: Box::new(local_expr(p, mir::Type::Class(c))),
                    index: 1,
                },
            ),
        )],
    );
    let module = lower(&b.finish(main));

    // The String field follows the 16-byte header and Int field,
    // so its natural byte offset is 24.
    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::HeapLoad { out, offset, .. } = instructions[0] else {
        panic!("a class field read must be a heap object load")
    };
    assert_eq!(*offset, 24);
    assert_eq!(function.temps[*out].ty, lir::MANAGED_PTR);
}

#[test]
fn class_init_allocates_and_stores_fields() {
    // The ctor body mir-lower generates for
    // `class Point(val x: Int, val s: String)`:
    // `return ClassInit Point [x, s]` — allocation plus one heap
    // store per flattened field.
    let mut b = Builder::new();
    let str_x = b.string("x");
    let point = b.class(
        "Point",
        None,
        &[("x", mir::Type::Int), ("s", mir::Type::String)],
        empty_vtable(),
        vec![],
    );
    let mut ctor_locals = Arena::new();
    let x = ctor_locals.alloc(local("x", mir::Type::Int));
    let s = ctor_locals.alloc(local("s", mir::Type::String));
    let ctor = b.user_fn_body(
        "ctor.Point",
        "scoop.ctor.Point",
        vec![
            param("x", mir::Type::Int, x),
            param("s", mir::Type::String, s),
        ],
        mir::Type::Class(point),
        returning_body(
            ctor_locals,
            expr(
                mir::Type::Class(point),
                mir::ExprKind::ClassInit {
                    class_id: point,
                    args: vec![
                        local_expr(x, mir::Type::Int),
                        local_expr(s, mir::Type::String),
                    ],
                },
            ),
        ),
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(point)));
    let main = b.main(
        locals,
        vec![call_value(
            p,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(ctor),
                },
                args: vec![mir::Expr::int(1), string_expr(str_x)],
            },
        )],
    );
    let module = lower(&b.finish(main));

    // `scoop_rt_alloc(td, size)` with the class layout size (16
    // header + Int @16 + String @24 = 32), then the fields at
    // those byte offsets.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "x"
  fun @scoop.ctor.Point(i64, ptr<managed>) -> ptr<managed>
  block entry
    poll managed-void-target0 sp3 live=[param1:ptr<managed>@0]
    call managed-direct-target0 sp1 live=[param1:ptr<managed>@0] t0 = sig=direct0 (ptr<metadata>, i64) -> ptr<managed> runtime @scoop_rt_alloc(td0, 32)
    heap_store t0 +16 param0
    heap_store t0 +24 param1
    ret t0
  fun @scoop_main() -> void
    local %0 p: ptr<managed>
  block entry
    poll managed-void-target0 sp4 live=[]
    call managed-direct-target0 sp2 live=[] t0 = sig=direct0 (i64, ptr<managed>) -> ptr<managed> local-fn0(1, global0)
    store t0 -> local0
    ret
  td td0 Point @scoop_td_Point type-id=2 size=32 parent=none vtable=[] itables=[]
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Point size=32 align=8 refs=[24]
  entry @scoop_main
"###);
}

#[test]
fn field_set_lowers_to_a_heap_store() {
    // `p.y = 3`: MIR FieldSet index 1 → byte offset 24 after the
    // 16-byte header and the first Int field.
    let mut b = Builder::new();
    let c = b.class(
        "C",
        None,
        &[("x", mir::Type::Int), ("y", mir::Type::Int)],
        empty_vtable(),
        vec![],
    );
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Class(c)));
    let main = b.main(
        locals,
        vec![stmt(mir::StatementKind::FieldSet {
            object: local_expr(p, mir::Type::Class(c)),
            index: 1,
            value: mir::Expr::int(3),
        })],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::HeapStore {
        object,
        offset: 24,
        value,
    } = instructions[0]
    else {
        panic!("a FieldSet must lower to a HeapStore")
    };
    assert!(matches!(object, lir::Value::Local(_)));
    assert!(matches!(value, lir::Value::IntConst(3)));
}

#[test]
fn a_trap_only_body_seals_the_function() {
    // mir-lower's abstract-method stub is a single trap call: the
    // block is sealed by the trap branch, so the "non-Unit
    // functions must end with `return`" check must not fire (it
    // applies to hir-lower-produced bodies that fall off the end,
    // not to noreturn bodies like this one).
    let mut b = Builder::new();
    let message = b.string("call to abstract method `Base.id`");
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", mir::Type::Any));
    let _stub = b.user_fn_full(
        "Base.id",
        "scoop.Base.id",
        vec![param("this", mir::Type::Any, this)],
        mir::Type::Int,
        locals,
        vec![call_stmt(runtime_call(
            mir::RuntimeFn::Trap,
            vec![string_expr(message)],
        ))],
    );
    let main = b.main(Arena::new(), vec![]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    assert!(matches!(
        function.blocks[function.entry].terminator,
        lir::Terminator::Br(_)
    ));
}
