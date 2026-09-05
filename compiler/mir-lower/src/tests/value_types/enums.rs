use super::super::*;

#[test]
fn enum_instances_are_created_once_with_substituted_fields() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    // A non-generic enum.
    let color_variants = ["Red", "Green", "Blue"]
        .iter()
        .map(|name| hir::Variant {
            name: name.to_string(),
            fields: Vec::new(),
        })
        .collect();
    let color = h.declare_enum("Color", Vec::new(), Vec::new(), color_variants);
    let color_ty = h.enum_ty(color);
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let option_int = h.option(int);
    let option_string = h.option(string);
    let option_s = h.option(s_ty);
    // f1 holds Option<Int> and Color; f2 holds Option<Int> again
    // (a duplicate request) and Option<String>.
    let mut locals1 = Arena::new();
    locals1.alloc(local("o", option_int));
    locals1.alloc(local("c", color_ty));
    let _f1 = h.user_fn(
        "f1",
        hir::Body {
            locals: locals1,
            statements: Vec::new(),
        },
    );
    let mut locals2 = Arena::new();
    locals2.alloc(local("o", option_int));
    locals2.alloc(local("s", option_string));
    let _f2 = h.user_fn(
        "f2",
        hir::Body {
            locals: locals2,
            statements: Vec::new(),
        },
    );
    let mut locals3 = Arena::new();
    locals3.alloc(local("s", option_s));
    let _f3 = h.user_fn(
        "f3",
        hir::Body {
            locals: locals3,
            statements: Vec::new(),
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    // One definition per (enum, type args), in creation order; the
    // duplicate Option<Int> request was deduplicated by enum identity.
    let names: Vec<&str> = module
        .enums
        .iter()
        .map(|(_, def)| def.name.as_str())
        .collect();
    assert_eq!(names, ["Option$I", "Color", "Option$S", "Option$D1_SX"]);

    // The variant field types are substituted with the instance's
    // type arguments.
    let option_int_def = &module.enums[la_arena::Idx::from_raw(0.into())];
    assert_eq!(option_int_def.variants[0].name, "Some");
    assert_eq!(option_int_def.variants[0].fields[0].ty, mir::Type::Int);
    assert!(option_int_def.gc_free);
    assert!(
        option_int_def
            .variants
            .iter()
            .all(|variant| variant.gc_free)
    );
    let option_string_def = &module.enums[la_arena::Idx::from_raw(2.into())];
    assert_eq!(
        option_string_def.variants[0].fields[0].ty,
        mir::Type::String
    );
    let option_s_def = &module.enums[la_arena::Idx::from_raw(3.into())];
    assert_eq!(
        option_s_def.variants[0].fields[0].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert_ne!(option_string_def.name, option_s_def.name);
    assert!(!option_string_def.gc_free);
    assert!(!option_string_def.variants[0].gc_free);
    assert!(option_string_def.variants[1].gc_free);
    // Color's variants are all unit variants.
    let color_def = &module.enums[la_arena::Idx::from_raw(1.into())];
    assert!(color_def.gc_free);
    assert_eq!(color_def.variants.len(), 3);
    assert!(
        color_def
            .variants
            .iter()
            .all(|variant| variant.fields.is_empty() && variant.gc_free)
    );
}

#[test]
fn option_nodes_become_generic_enum_operations() {
    let mut h = Harness::new();
    let (int, boolean) = (h.int, h.boolean);
    let option_int = h.option(int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let n = locals.alloc(local("n", option_int));
    let b = locals.alloc(local("b", boolean));
    let y = locals.alloc(local("y", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 41))),
                        option_int,
                    ),
                ),
                val_decl(n, expr(hir::ExprKind::NoneLiteral, option_int)),
                val_decl(
                    b,
                    expr(
                        hir::ExprKind::IsSome(Box::new(local_ref(o, option_int))),
                        boolean,
                    ),
                ),
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(local_ref(o, option_int)),
                            trap_on_none: false,
                        },
                        int,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 41
      val n: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v1
      val b: Boolean
        Type Boolean
        Binary MachineEq(EnumTag)
          Type machine<enum-tag>
          EnumTag
            Type Option$I<Int>
            Local o
          Type machine<enum-tag>
          MachineScalarLiteral EnumTag(0)
      val y: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local o
      return
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn trapping_unwrap_becomes_a_guarded_extraction() {
    // val o = Some(1); val y = o!!
    let mut h = Harness::new();
    h.exception("UnwrapException");
    let int = h.int;
    let option_int = h.option(int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let y = locals.alloc(local("y", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                        option_int,
                    ),
                ),
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(local_ref(o, option_int)),
                            trap_on_none: true,
                        },
                        int,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The operand is evaluated once into `$opt.1`; the tag test
    // guards the extraction, and the else branch throws
    // `UnwrapException()` (M8) — an ordinary constructor call.
    let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  class UnwrapException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 1
      val $opt.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary MachineEq(EnumTag)
          Type machine<enum-tag>
          EnumTag
            Type Option$I<Int>
            Local $opt.1
          Type machine<enum-tag>
          MachineScalarLiteral EnumTag(0)
    bb1 if.then.1
      val $uw.2: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $opt.1
      goto bb3
    bb2 if.else.2
      assign $new.1
        Type UnwrapException
        ClassAlloc UnwrapException
      call @scoop.init.UnwrapException.$c0 direct
        Type UnwrapException
        Local $new.1
      throw
        Type UnwrapException
        Local $new.1
    bb3 if.merge.3
      val y: Int
        Type Int
        Local $uw.2
      return
  fun init.UnwrapException.$c0 @scoop.init.UnwrapException.$c0(this: UnwrapException) -> Unit
    bb0 entry
      return
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}
