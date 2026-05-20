//! MIR lowering integration tests.

#![allow(dead_code)]

use super::*;

use crate::pipeline::TypedHirEffectContracts;
use crate::session::Session;
use crate::source::SourceFile;
use std::path::PathBuf;

#[test]
fn typed_contracts_clear_fallback_resume_and_perform_metadata() {
    let span = Span::new(1, 2);
    let fallback_effect_sites = std::iter::once((
        hir::CallSite::new(PathBuf::from("fixtures/mir_lower_facts.scoop"), span),
        hir::EffectOpCallInfo {
            arg_mapping: vec![0],
            payload_tuple_ty: None,
        },
    ))
    .collect::<hir::EffectOpCallSiteIndex>();
    let dispatch_sites = hir::DispatchCallSiteIndex::default();
    let when_pat_binding_tys = hir::WhenPatBindingTypeIndex::default();
    let top_level_fun_call_sites = hir::TopLevelFunCallSiteIndex::default();

    let facts = MirLoweringFacts::from_hir_side_tables_and_resume_spans(
        &dispatch_sites,
        [span],
        [span],
        &fallback_effect_sites,
        &when_pat_binding_tys,
        &top_level_fun_call_sites,
    )
    .with_typed_contracts(&TypedHirEffectContracts::default());

    assert!(facts.uses_typed_contracts());
    assert!(!facts.fallback_resume_site_matches(span));
    assert!(!facts.fallback_resume_site_suspends_outward(span));
    assert!(facts.fallback_perform_site_info(span).is_none());
}

#[test]
fn dump_mir_emits_top_level_initializer_and_extern_roots() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_top_level_roots.scoop",
        r#"
package sample

import scoop.core.*

val Base: Int = 1
val Runtime: Int = Base + 1

@Global
var Counter: Int = Runtime

@Extern(name = "native_counter")
var NativeCounter: Int

object Registry {
    val count: Int = Runtime
}

fun main() {}
"#,
    );

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let initializer_fqns = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::InitializerRoot(root) => Some(root.fqn.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for expected in [
        "sample.Base",
        "sample.Runtime",
        "sample.Counter",
        "sample.Registry",
    ] {
        assert!(
            initializer_fqns.contains(&expected),
            "dump-mir should publish initializer root `{expected}`: {initializer_fqns:?}"
        );
    }

    let runtime = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::InitializerRoot(root) if root.fqn == "sample.Runtime" => Some(root),
            _ => None,
        })
        .expect("runtime top-level val should publish initializer root");
    assert_eq!(runtime.kind, InitializerRootKind::RuntimeImmutableVal);
    assert!(runtime.dependencies.iter().any(|dependency| {
        dependency.fqn == "sample.Base"
            && dependency.kind == InitializerDependencyKind::TopLevelValue
    }));

    let native = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::ExternGlobal(root) if root.fqn == "sample.NativeCounter" => Some(root),
            _ => None,
        })
        .expect("dump-mir should publish extern global root");
    assert_eq!(native.symbol, "native_counter");
    assert!(native.initializer_absent);
}

#[test]
fn dump_mir_emits_type_body_generic_member_fun_roots() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_member_root_generic.scoop",
        r#"
package fixtures.mirlower

class Box() {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box): Int / E {
    return box.forward<eff E>()
}
"#,
    );

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun_fqns = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) => Some(fun.fqn.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        fun_fqns.contains(&"fixtures.mirlower.Box.forward"),
        "generic MIR lowering 应显式发射 type-body generic member fun root"
    );
    assert!(
        fun_fqns.contains(&"fixtures.mirlower.wrap"),
        "顶层 generic fun root 仍应继续保留"
    );
}

#[test]
fn dump_mir_emits_companion_generic_member_fun_roots() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_companion_member_root_generic.scoop",
        r#"
package fixtures.mirlower

class Box() {
    companion object {
        fun <eff E = Pure> forward(): Int / E {
            return 1
        }
    }
}

fun <eff E = Pure> wrap(): Int / E {
    return Box.forward<eff E>()
}
"#,
    );

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun_fqns = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) => Some(fun.fqn.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        fun_fqns.contains(&"fixtures.mirlower.Box.Companion.forward"),
        "generic MIR lowering 应显式发射 companion generic member fun root"
    );
    assert!(
        fun_fqns.contains(&"fixtures.mirlower.wrap"),
        "顶层 generic fun root 仍应继续保留"
    );
}

#[test]
fn dump_mir_types_comparison_condition_as_bool_in_generic_template() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_generic_compare_bool.scoop",
        r#"
package fixtures.mirlower

fun repeat<T>(x: T, n: Int): T {
    if (n <= 0) {
        return x
    }
    return repeat(x, n - 1)
}
"#,
    );

    let mut lowered = lower_for_dump(&sess, &source).unwrap();
    let builtins = lowered.types.intern_builtins();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.mirlower.repeat" => Some(fun),
            _ => None,
        })
        .expect("expected generic repeat MIR root");
    let body = fun.body.as_ref().expect("repeat should have a MIR body");
    let TerminatorKind::CondBr { cond, .. } = &body.blocks[body.start.as_usize()].terminator.kind
    else {
        panic!("expected repeat entry block to branch on comparison");
    };
    let Operand::Local(cond_local) = cond else {
        panic!("comparison condition should be stored in a local");
    };
    let cond_ty = body.locals[cond_local.as_u32() as usize].ty;

    assert_eq!(
        cond_ty, builtins.bool_,
        "MIR comparison result local should be Bool, not an overly broad fallback type"
    );
}

#[test]
fn dump_mir_lowers_user_defined_compare_to_as_direct_call_plus_int_compare_method() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_compare_to_direct_call.scoop",
        r#"
package fixtures.mirlower

struct Num(val value: Int) {
    fun compareTo(other: Num): Int {
        return this.value - other.value
    }
}

fun entry(lhs: Num, rhs: Num): Bool {
    return lhs < rhs
}
"#,
    );

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.mirlower.entry" => Some(fun),
            _ => None,
        })
        .expect("expected entry MIR root");
    let body = fun.body.as_ref().expect("entry should have a MIR body");
    let entry_block = &body.blocks[body.start.as_usize()];
    let compare_to_call_targets = entry_block
        .stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            StatementKind::Assign {
                target,
                value:
                    Rvalue::Call {
                        kind: CallKind::Direct { callee_fqn },
                        args,
                        ..
                    },
                ..
            } if callee_fqn == "fixtures.mirlower.Num.compareTo" && args.len() == 2 => {
                Some(*target)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        compare_to_call_targets.len(),
        1,
        "generic MIR compareTo lowering 不应重复套用 compareTo 语法糖"
    );
    let zero_locals = entry_block
        .stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            StatementKind::Assign {
                target,
                value: Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
                ..
            } => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(
        !compare_to_call_targets.is_empty(),
        "generic MIR compareTo lowering 应显式发射 direct-call target"
    );
    assert!(
        entry_block.stmts.iter().any(|stmt| {
            if let StatementKind::Assign {
                value:
                    Rvalue::Call {
                        kind: CallKind::Direct { callee_fqn },
                        args,
                        ..
                    },
                ..
            } = &stmt.kind
            {
                callee_fqn == "scoop.core.Int.lt"
                    && args.len() == 2
                    && matches!(args[0].value, Operand::Local(local) if local == compare_to_call_targets[0])
                    && matches!(args[1].value, Operand::Local(local) if zero_locals.contains(&local))
            } else {
                false
            }
        }),
        "compareTo direct-call 结果应继续进入 Int.lt method intrinsic 比较主线"
    );
    assert!(
        !zero_locals.is_empty(),
        "compareTo → 0 比较应在 MIR 中保留显式的合成整数常量"
    );
}

#[test]
fn dump_mir_lowers_compare_to_in_if_condition_as_direct_call() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_compare_to_if_condition.scoop",
        r#"
package fixtures.mirlower

struct Num(val value: Int) {
    fun compareTo(other: Num): Int {
        return this.value - other.value
    }
}

fun entry(lhs: Num, rhs: Num): Int {
    if (lhs < rhs) {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.mirlower.entry" => Some(fun),
            _ => None,
        })
        .expect("expected entry MIR root");
    let body = fun.body.as_ref().expect("entry should have a MIR body");
    let compare_to_call_targets = body
        .blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .filter_map(|stmt| match &stmt.kind {
            StatementKind::Assign {
                target,
                value:
                    Rvalue::Call {
                        kind: CallKind::Direct { callee_fqn },
                        args,
                        ..
                    },
                ..
            } if callee_fqn == "fixtures.mirlower.Num.compareTo" && args.len() == 2 => {
                Some(*target)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let zero_locals = body
        .blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .filter_map(|stmt| match &stmt.kind {
            StatementKind::Assign {
                target,
                value: Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
                ..
            } => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        compare_to_call_targets.len() == 1,
        "if 条件里的 compareTo 比较也应显式发射 direct-call target"
    );
    assert!(
        body.blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .any(|stmt| {
                if let StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                    ..
                } = &stmt.kind
                {
                    callee_fqn == "scoop.core.Int.lt"
                        && args.len() == 2
                        && matches!(args[0].value, Operand::Local(local) if local == compare_to_call_targets[0])
                        && matches!(args[1].value, Operand::Local(local) if zero_locals.contains(&local))
                } else {
                    false
                }
            }),
        "if 条件里的 compareTo → 0 比较应调用 Int.lt method intrinsic"
    );
}

#[test]
fn dump_mir_lowers_safe_member_access_option_result_without_ctor_todo() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_safe_member_access_option_result.scoop",
        r#"
package fixtures.mirlower

import scoop.core.*

class User(val score: Int)

fun entry(user: User?): Int? {
    return user?.score
}
"#,
    );

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.mirlower.entry" => Some(fun),
            _ => None,
        })
        .expect("expected entry MIR root");
    let body = fun.body.as_ref().expect("entry should have a MIR body");

    assert!(
        body.blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .all(|stmt| {
                !matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Todo(_),
                        ..
                    }
                )
            }),
        "safe member access desugar 应通过 Option variant ctor/value 主线，而不是留下任意 Rvalue Todo"
    );

    let mut saw_some = false;
    let mut saw_none = false;
    for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
        let StatementKind::Assign {
            value: Rvalue::EnumVariant {
                variant_name, args, ..
            },
            ..
        } = &stmt.kind
        else {
            continue;
        };
        match (variant_name.as_str(), args.len()) {
            ("Some", 1) => saw_some = true,
            ("None", 0) => saw_none = true,
            _ => {}
        }
    }

    assert!(
        saw_some,
        "safe member access 的 Some 分支应 lower 为 Option.Some ctor"
    );
    assert!(
        saw_none,
        "safe member access 的 None 分支应 lower 为 Option.None ctor/value"
    );
}

#[test]
fn dump_mir_smart_cast_member_access_preserves_concrete_generic_field_type() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_smart_cast_generic_member_access.scoop",
        r#"
package fixtures.mirlower

import scoop.core.*

class Box<T>(val value: T)

fun readValue(x: Any): Int {
    if (x is Box<Int>) {
        return x.value
    }
    return 0
}
"#,
    );

    let mut lowered = lower_for_dump(&sess, &source).unwrap();
    let builtins = lowered.types.intern_builtins();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.mirlower.readValue" => Some(fun),
            _ => None,
        })
        .expect("expected readValue MIR root");
    let body = fun.body.as_ref().expect("readValue should have a MIR body");
    let param_local = fun.params[0].local;
    let (receiver_local, target_local, member_receiver_ty) = body
        .blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .find_map(|stmt| match &stmt.kind {
            StatementKind::Assign {
                target,
                value:
                    Rvalue::MemberAccess {
                        receiver: Operand::Local(receiver_local),
                        member,
                        ..
                    },
            } => Some((*receiver_local, *target, member.receiver_ty)),
            _ => None,
        })
        .expect("smart-cast branch should lower to a member access statement");

    assert_eq!(
        body.locals[target_local.as_u32() as usize].ty,
        builtins.int,
        "smart-cast branch的 member access 结果 local 应保持 concrete Int，而不是声明处的 T"
    );
    assert_ne!(
        receiver_local, param_local,
        "smart-cast branch 应为 narrowed receiver 建立单独 local，不能继续直接复用 Any 形参槽位"
    );
    assert_eq!(
        body.locals[receiver_local.as_u32() as usize].ty,
        member_receiver_ty,
        "member metadata 的 receiver_ty 应与 narrowed receiver local 一致"
    );
    match lowered
        .types
        .kind(body.locals[receiver_local.as_u32() as usize].ty)
    {
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            assert_eq!(
                nominal.args,
                vec![builtins.int],
                "smart-cast receiver local 应具体化为 Box<Int>"
            );
        }
        other => panic!("expected narrowed receiver local to be Box<Int>, got {other:?}"),
    }
}

#[test]
fn typed_hir_fixture_preserves_compare_to_direct_call_binding() {
    let sess = Session::new().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/operator_overload_struct_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let lowered = crate::hir::lower_typed_for_dump(&sess, &source).unwrap();
    assert!(
        lowered
            .top_level_fun_call_sites
            .values()
            .any(|binding| binding.fqn == "Num.compareTo"),
        "typed HIR side table 应保留 fixture compareTo 站点的 direct-call binding"
    );
}

#[test]
fn dump_mir_publishes_member_write_contract_for_escape_continuation_cell() {
    let sess = Session::new().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "main" => Some(fun),
            _ => None,
        })
        .expect("expected main MIR root");
    let body = fun.body.as_ref().expect("main should have a MIR body");

    assert!(
        body.blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .all(|stmt| !matches!(stmt.kind, StatementKind::Todo(_))),
        "member writes should no longer leak statement Todo"
    );

    let mut saw_some_k_write = false;
    let mut saw_none_write = false;
    for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
        let StatementKind::StoreMember {
            member,
            continuation_route,
            ..
        } = &stmt.kind
        else {
            continue;
        };
        let Some(MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
            continue;
        };
        if fqn != "Cell.k" {
            continue;
        }
        match continuation_route {
            StoredContinuationRoutePublication::Unique(route)
                if matches!(
                    route.path.as_slice(),
                    [PatternBindingStep::VariantField {
                        variant,
                        field_index: 0,
                    }] if variant == "Some"
                ) =>
            {
                saw_some_k_write = true;
            }
            StoredContinuationRoutePublication::None => {
                saw_none_write = true;
            }
            StoredContinuationRoutePublication::Ambiguous
            | StoredContinuationRoutePublication::Unique(_) => {}
        }
    }

    assert!(
        saw_some_k_write,
        "cell.k = Some(k) 应发布 wrapper path + source local 的 continuation write contract"
    );
    assert!(
        saw_none_write,
        "cell.k = none_k 应发布显式 member write contract，而不是 TODO"
    );
}

#[test]
fn dump_mir_nested_uint8_array_literals_keep_expected_element_type() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_nested_uint8_array_expected_type.scoop",
        r#"
package sample

import scoop.core.*

fun takeByte(xs: Array<UInt8>): UInt8 {
    return xs.get(0)
}

fun main(): Int {
    val bytesIf: Array<UInt8> = [if (true) { 1 + 2 } else { 9 }]
    val bytesWhen: Array<UInt8> = [when (false) {
        true -> 7
        false -> 4
    }]
    val argByte: UInt8 = takeByte([if (false) { 7 } else { 4 }])
    return 0
}
"#,
    );

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "sample.main" => Some(fun),
            _ => None,
        })
        .expect("expected main MIR root");
    let body = fun.body.as_ref().expect("main should have a MIR body");

    let mut seen_uint8_pushes = 0;
    for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
        let StatementKind::Assign {
            value:
                Rvalue::Call {
                    kind: CallKind::Direct { callee_fqn },
                    args,
                    transport,
                    ..
                },
            ..
        } = &stmt.kind
        else {
            continue;
        };
        if intrinsic_base_fqn(callee_fqn) != "scoop.core.push" {
            continue;
        }
        let array = transport
            .array
            .as_ref()
            .expect("MutableArray.push should publish array transport metadata");
        if lowered.types.display(array.element_ty).to_string() != "UInt8" {
            continue;
        }
        let value_local = match args.get(1).map(|arg| &arg.value) {
            Some(Operand::Local(local)) => *local,
            _ => panic!("MutableArray.push value should stay in a local"),
        };
        assert_eq!(
            lowered
                .types
                .display(body.locals[value_local.as_u32() as usize].ty)
                .to_string(),
            "UInt8",
            "nested UInt8 array literal element local should keep UInt8 expected type"
        );
        assert_eq!(
            lowered.types.display(array.element.source_ty).to_string(),
            "UInt8",
            "nested UInt8 array literal transport should keep UInt8 source surface"
        );
        assert_eq!(
            array.element.kind,
            MirTransportKind::ArrayElement,
            "nested UInt8 array literal transport should stay on array-element path"
        );
        assert!(
            !array.element.requirements.trace,
            "nested UInt8 array literal should not claim trace metadata"
        );
        assert!(
            !array.element.requirements.drop,
            "nested UInt8 array literal should not claim aggregate drop obligations"
        );
        assert!(
            array.element.boxing.is_none(),
            "nested UInt8 array literal should not publish composite boxing metadata"
        );
        seen_uint8_pushes += 1;
    }

    assert_eq!(
        seen_uint8_pushes, 3,
        "expected UInt8 MutableArray.push sites for if / when / call-arg nested array literals"
    );
}

#[test]
fn mir_array_literal_helper_calls_keep_distinct_call_contracts() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/mir_array_literal_helper_call_contracts.scoop",
        r#"
package sample

import scoop.core.*

struct Point(val x: Int, val y: Int)

enum Item {
    Hit(val point: Point),
    Pair(val payload: (Point, Int)),
}

fun main(): Int {
    val items: MutableArray<Item> = [Hit(Point(1, 2)), Pair((Point(3, 4), 5))]
    return when (items.get(0)) {
        Hit(point) -> point.x + point.y
        Pair(payload) -> payload._0.x + payload._0.y + payload._1
    }
}
"#,
    );

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "sample.main" => Some(fun),
            _ => None,
        })
        .expect("expected main MIR root");
    let body = fun.body.as_ref().expect("main should have a MIR body");

    let mut array_pushes = 0;
    let mut saw_hit_variant = false;
    let mut saw_pair_variant = false;

    for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
        let StatementKind::Assign { value, .. } = &stmt.kind else {
            continue;
        };
        match value {
            Rvalue::Call {
                kind: CallKind::Direct { callee_fqn },
                args,
                transport,
                ..
            } if intrinsic_base_fqn(callee_fqn) == "scoop.core.push" => {
                array_pushes += 1;
                assert_eq!(
                    args.len(),
                    2,
                    "MutableArray.push must keep receiver + element args instead of stealing an element contract"
                );
                let value_local = match args.get(1).map(|arg| &arg.value) {
                    Some(Operand::Local(local)) => *local,
                    _ => panic!("MutableArray.push element should stay in a local"),
                };
                assert_eq!(
                    lowered
                        .types
                        .display(body.locals[value_local.as_u32() as usize].ty)
                        .to_string(),
                    "sample.Item",
                    "MutableArray.push element local should keep the enum element surface"
                );
                let array = transport
                    .array
                    .as_ref()
                    .expect("MutableArray.push should publish array transport metadata");
                assert_eq!(
                    lowered.types.display(array.element_ty).to_string(),
                    "sample.Item",
                    "MutableArray.push element type should remain the enum surface"
                );
            }
            Rvalue::EnumVariant { variant_name, .. } if variant_name == "Hit" => {
                saw_hit_variant = true;
            }
            Rvalue::EnumVariant { variant_name, .. } if variant_name == "Pair" => {
                saw_pair_variant = true;
            }
            _ => {}
        }
    }

    assert_eq!(
        array_pushes, 2,
        "expected exactly two MutableArray.push calls for the two array literal elements"
    );
    assert!(
        saw_hit_variant,
        "enum element `Hit(...)` should remain an EnumVariant rvalue instead of being mis-lowered as array builder helper"
    );
    assert!(
        saw_pair_variant,
        "enum element `Pair(...)` should remain an EnumVariant rvalue instead of being mis-lowered as array builder helper"
    );
}

#[test]
fn dump_mir_uint8_array_get_keeps_scalar_transport_metadata() {
    let sess = Session::new().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/literal_numeric_expected_type_absorption_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let lowered = lower_for_dump(&sess, &source).unwrap();
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "main" => Some(fun),
            _ => None,
        })
        .expect("expected main MIR root");
    let body = fun.body.as_ref().expect("main should have a MIR body");

    let mut seen_uint8_gets = 0;
    for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
        let StatementKind::Assign {
            value:
                Rvalue::Call {
                    kind: CallKind::Direct { callee_fqn },
                    transport,
                    ..
                },
            ..
        } = &stmt.kind
        else {
            continue;
        };
        if callee_fqn != "scoop.core.get" && callee_fqn != "scoop.core.Array.get" {
            continue;
        }
        if stmt.span != Span::new(1062, 1074) && stmt.span != Span::new(1106, 1118) {
            continue;
        }
        let array = transport
            .array
            .as_ref()
            .expect("UInt8 array get should publish array transport metadata");
        assert_eq!(
            array.operation,
            ArrayTransportOperation::Get,
            "direct bytes.get call should stay on get transport metadata"
        );
        assert_eq!(
            transport.result.kind,
            MirTransportKind::Scalar,
            "UInt8 array get result should stay on scalar transport path"
        );
        assert!(
            lowered
                .types
                .display(transport.result.source_ty)
                .to_string()
                .ends_with("UInt8"),
            "UInt8 array get result should still surface as UInt8"
        );
        assert!(
            !transport.result.requirements.trace,
            "UInt8 array get result should not require trace metadata"
        );
        assert!(
            !transport.result.requirements.drop,
            "UInt8 array get result should not claim aggregate drop requirements"
        );
        assert!(
            transport.aggregate_return.is_none(),
            "UInt8 array get should not publish aggregate return metadata"
        );
        assert!(
            !array.element.requirements.trace,
            "UInt8 array get element transport should stay on scalar path"
        );
        assert!(
            !array.element.requirements.drop,
            "UInt8 array get element transport should not claim aggregate drop obligations"
        );
        seen_uint8_gets += 1;
    }

    assert_eq!(
        seen_uint8_gets, 2,
        "expected direct bytes.get compare path to retain two UInt8 scalar get sites"
    );
}
