use super::*;

#[test]
fn constructor_functions_initialize_the_flattened_fields() {
    // open class Root(val label: String)
    // open class Base(val name: String) : Root("root")
    // class Point(val x: Int) : Base("point")
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let root = h.class(
        "Root",
        hir::ClassModifier::Open,
        &[("label", string)],
        None,
        &[],
    );
    let base = h.class(
        "Base",
        hir::ClassModifier::Open,
        &[("name", string)],
        Some((root, vec![str_lit(&h, "root")])),
        &[],
    );
    let point = h.class(
        "Point",
        hir::ClassModifier::Final,
        &[("x", int)],
        Some((base, vec![str_lit(&h, "point")])),
        &[],
    );
    let point_ty = h.class_ty(point);
    let point_application = h.class_application_of(point_ty);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", point_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                p,
                expr(
                    hir::ExprKind::ClassInit {
                        application: point_application,
                        args: vec![int_lit(&h, 1)],
                    },
                    point_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    // One ctor per class; the use site is a plain direct call.
    // Each ctor returns a raw ClassInit over the flattened field
    // values: the base delegation arguments (re-evaluated in each
    // derived ctor — hence the repeated "root" constant), then the
    // own properties. No base ctor is called.
    let expected = "\
Module
  class Root vtable=0 itables=0
  class Base vtable=0 itables=0
  class Point vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      call p: Point = @scoop.ctor.Point direct
        Type Int
        IntLiteral 1
      return
  fun ctor.Root @scoop.ctor.Root(label: String) -> Root
    bb0 entry
      return
        Type Root
        ClassInit Root
          Type String
          Local label
  fun ctor.Base @scoop.ctor.Base(name: String) -> Base
    bb0 entry
      return
        Type Base
        ClassInit Base
          Type String
          StringConst @scoop.str.0
          Type String
          Local name
  fun ctor.Point @scoop.ctor.Point(x: Int) -> Point
    bb0 entry
      return
        Type Point
        ClassInit Point
          Type String
          StringConst @scoop.str.2
          Type String
          StringConst @scoop.str.1
          Type Int
          Local x
  str @scoop.str.0 \"root\"
  str @scoop.str.1 \"point\"
  str @scoop.str.2 \"root\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn abstract_classes_get_no_constructor() {
    let mut h = Harness::new();
    let _base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert!(
        !module
            .functions
            .iter()
            .any(|(_, f)| f.symbol == "scoop.ctor.Base")
    );
}

#[test]
fn field_assignment_lowers_to_field_set() {
    // `p.y = 3` on a class with two properties (index 1 in the
    // flattened layout).
    let mut h = Harness::new();
    let int = h.int;
    let c = h.class(
        "C",
        hir::ClassModifier::Final,
        &[("x", int), ("y", int)],
        None,
        &[],
    );
    let c_ty = h.class_ty(c);
    let c_application = h.class_application_of(c_ty);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", c_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Field {
                    receiver: Box::new(local_ref(p, c_ty)),
                    field: hir::FieldRef::ClassField {
                        application: c_application,
                        index: 1,
                    },
                },
                value: int_lit(&h, 3),
            })],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let mir::StatementKind::FieldSet {
        object,
        index: 1,
        value,
    } = &entry_statements(body)[0].kind
    else {
        panic!("a class property assignment must lower to FieldSet")
    };
    assert!(matches!(object.kind, mir::ExprKind::Local(_)));
    assert!(matches!(value.kind, mir::ExprKind::IntLiteral(3)));
}
