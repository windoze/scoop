use super::*;

#[test]
fn class_initializers_chain_on_one_exact_allocation() {
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
    let point_constructor = h
        .class_constructor_applications
        .iter()
        .find_map(|(id, app)| (app.constructor == h.classes[point].constructors[0]).then_some(id))
        .expect("point primary constructor application");
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
                        constructor: point_constructor,
                        args: vec![int_lit(&h, 1)],
                    },
                    point_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    // The use site performs the only allocation. Each initializer receives
    // that object, calls the direct base initializer, then writes its own
    // complete-layout field.
    let expected = "\
Module
  class Root vtable=0 itables=0
  class Base vtable=0 itables=0
  class Point vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      assign $new.1
        Type Point
        ClassAlloc Point
      call @scoop.init.C5_PointX.$c2 direct
        Type Point
        Local $new.1
        Type Int
        IntegerLiteral Int value=1 bits=0x00000001
      val p: Point
        Type Point
        Local $new.1
      return
  fun init.Root.$c0 @scoop.init.C4_RootX.$c0(this: Root, label: String) -> Unit
    bb0 entry
      field_set 0
        Type Root
        Local this
        Type String
        Local label
      return
  fun init.Base.$c1 @scoop.init.C4_BaseX.$c1(this: Base, name: String) -> Unit
    bb0 entry
      call @scoop.init.C4_RootX.$c0 direct
        Type Root
        Retype Root
          Type Base
          Local this
        Type String
        StringConst @scoop.str.0
      field_set 1
        Type Base
        Local this
        Type String
        Local name
      return
  fun init.Point.$c2 @scoop.init.C5_PointX.$c2(this: Point, x: Int) -> Unit
    bb0 entry
      call @scoop.init.C4_BaseX.$c1 direct
        Type Base
        Retype Base
          Type Point
          Local this
        Type String
        StringConst @scoop.str.1
      field_set 2
        Type Point
        Local this
        Type Int
        Local x
      return
  str @scoop.str.0 \"root\"
  str @scoop.str.1 \"point\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn abstract_classes_keep_an_initializer_for_derived_delegation() {
    let mut h = Harness::new();
    let _base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert!(
        module
            .functions
            .iter()
            .any(|(_, f)| f.symbol == "scoop.init.C4_BaseX.$c0")
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
    let y_field = h.classes[c].fields[1];
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
                        field: y_field,
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
    assert!(matches!(
        value.kind,
        mir::ExprKind::IntegerLiteral(mir::MirIntegerConstant::Signed32(3))
    ));
}
