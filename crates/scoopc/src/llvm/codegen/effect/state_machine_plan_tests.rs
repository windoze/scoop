mod tests {
    use std::collections::{HashMap, HashSet};

    use crate::ast;
    use crate::hir;
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore};
    use crate::typecheck;

    use super::{
        HandleBranchCondition, HandlePlanContext, HandleSegmentList, HandleSegmentTerminator,
        HandleStateMachinePlan, HandleStateOp, ImmediateResumeFrame, MainCodegen,
        MixedEscapeDirectFrame, ResumeFrame, StateTerminator,
    };

    #[test]
    fn plan_dump_covers_direct_branch_loop_and_finally() {
        let dump = build_plan_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var sum: Int = 0
        if (flag) {
            val x: Int = Yield.next()
            sum = x
        } else {
            sum = 1
        }
        while (sum < 3) {
            sum = sum + 1
        }
        sum
    } with {
        Yield.next() -> resume {
            resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=direct-perform"));
        assert!(dump.contains("branch cond=if-cond"));
        assert!(dump.contains("loop re-entry"));
        assert!(dump.contains("cleanup0 kind=finally"));
        assert!(dump.contains("mode=immediate-resume"));
        assert!(dump.contains("path=top[1] -> if-then[0]"));
    }

    #[test]
    fn plan_dump_records_single_immediate_resume_while_body_path() {
        let dump = build_plan_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    val result: Int = handle {
        var sum: Int = 0
        while (sum == 0) {
            val x: Int = Yield.next()
            sum = x
        }
        sum
    } with {
        Yield.next() -> resume {
            resume(7)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=direct-perform"));
        assert!(dump.contains("path=top[1] -> while-body[0]"));
    }

    #[test]
    fn plan_dump_distinguishes_state_machine_callee_and_indirect_call_sites() {
        let dump = build_plan_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(thunk: () -> Int / (Ask)): Int {
    val result: Int = handle {
        val a: Int = fetch(1)
        val b: Int = thunk()
        a + b
    } with {
        Ask.ask(seed) -> resume {
            resume(seed + 10)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-state-machine-callee"));
        assert!(dump.contains("detail=a.fetch"));
        assert!(dump.contains("kind=indirect-call-may-suspend"));
        assert!(dump.contains("path=top[0]"));
        assert!(dump.contains("path=top[1]"));
    }

    #[test]
    fn plan_dump_indirect_call_captures_call_site_reads() {
        let dump = build_plan_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(): Int {
    val result: Int = handle {
        val base: Int = 1
        val value: Int = fetch(base)
        base + value
    } with {
        Ask.ask(seed: Int), k -> 0
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-state-machine-callee"));
        assert!(dump.contains("captures=[base#"), "{dump}");
        assert!(dump.contains("path=top[1]"));
    }

    #[test]
    fn plan_dump_covers_nested_handle_and_multiple_arms() {
        let dump = build_plan_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Nothing
}

fun demo(mode: Int): Int {
    val result: Int = handle {
        val inner: Int = handle {
            val x: Int = Yield.next()
            x + mode
        } with {
            Yield.next() -> resume {
                resume(10)
            }
        }
        if (mode == 0) {
            val y: Int = Ask.current()
            inner + y
        } else {
            Boom.boom(mode)
            0
        }
    } with {
        Ask.current(), k -> 7
        Boom.boom(code: Int) -> 0
    }
    result
}
"#,
        );

        assert!(dump.contains("nested-handles:\n  nested#0"));
        assert!(dump.contains("mode=escape-continuation"));
        assert!(dump.contains("mode=never-resume"));
        assert!(dump.contains("dispatch:\n  a.Ask.current => [arm0]\n  a.Boom.boom => [arm1]"));
    }

    #[test]
    fn simplification_dump_marks_never_resume_as_flag_unwind() {
        let dump = build_mode_specific_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Ask.ask(1)
        x + 1
    } with {
        Ask.ask(seed: Int) -> seed + 10
    }
    result
}
"#,
        );

        assert!(dump.contains("payload=yes"));
        assert!(dump.contains("stack-reentry=no"));
        assert!(dump.contains("heap-continuation=no"));
        assert!(dump.contains("lowering=flag-unwind"));
        assert!(dump.contains("target=-"));
    }

    #[test]
    fn simplification_dump_marks_immediate_resume_as_stack_reentry() {
        let dump = build_mode_specific_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Ask.ask(1)
        x + 1
    } with {
        Ask.ask(seed: Int) -> resume {
            resume(seed + 10)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("stack-reentry=yes"));
        assert!(dump.contains("heap-continuation=no"));
        assert!(dump.contains("one-shot=no"));
        assert!(dump.contains("lowering=stack-reenter"));
        assert!(dump.contains("target=s"));
    }

    #[test]
    fn simplification_dump_marks_escape_continuation_as_heap_materialization() {
        let dump = build_mode_specific_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Ask.ask(1)
        x + 1
    } with {
        Ask.ask(seed: Int), k -> seed + 10
    }
    result
}
"#,
        );

        assert!(dump.contains("stack-reentry=no"));
        assert!(dump.contains("heap-continuation=yes"));
        assert!(dump.contains("one-shot=yes"));
        assert!(dump.contains("lowering=heap-continuation"));
        assert!(dump.contains("target=s"));
    }

    #[test]
    fn resolve_escape_direct_sites_from_plan_recovers_nested_paths() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Suspend {
    fun fetch(): Int
}

fun demo(): Unit {
    val _: Unit = handle {
        val threshold: Int = Suspend.fetch()
        while (threshold > 0) {
            if (threshold > 1) {
                val bonus: Int = Suspend.fetch()
                println(bonus)
            }
        }
    } with {
        Suspend.fetch(), k -> { () }
    }
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let resolved = MainCodegen::resolve_escape_direct_sites_from_plan(
            handle,
            &plan,
            0,
            &handle.arms[0].op.op.fqn,
        )
        .expect("plan-driven direct escape resolution should succeed");

        assert_eq!(resolved.perform_sites.len(), 2);
        assert!(resolved.perform_sites[0].resume_path.is_empty());
        assert!(matches!(
            resolved.perform_sites[1].resume_path.as_slice(),
            [ResumeFrame::WhileBody { .. }, ResumeFrame::IfThen { .. }]
        ));
    }

    #[test]
    fn resolve_escape_indirect_sites_from_plan_preserves_call_site_captures() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(): Int {
    val result: Int = handle {
        val base: Int = 1
        val value: Int = fetch(base)
        base + value
    } with {
        Ask.ask(seed: Int), k -> 0
    }
    result
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let resolved = MainCodegen::resolve_escape_indirect_sites_from_plan(handle, &plan)
            .expect("plan-driven indirect escape resolution should succeed");
        let base_id = find_handle_local_id_by_name(handle, "base").expect("expected local `base`");

        assert_eq!(resolved.indirect_sites.len(), 1);
        assert_eq!(resolved.indirect_sites[0].stmt_idx, 1);
        assert!(resolved.capture_ids.contains(&base_id));
    }

    #[test]
    fn resolve_escape_direct_sites_from_plan_captures_prior_resumed_ref_local() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

class Box(val value: Int)

effect Provide {
    fun provide(): Box
}

fun demo(): Unit {
    val _: Unit = handle {
        val b1: Box = Provide.provide()
        val b2: Box = Provide.provide()
        println(b1.value + b2.value)
    } with {
        Provide.provide(), k -> { () }
    }
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let resolved = MainCodegen::resolve_escape_direct_sites_from_plan(
            handle,
            &plan,
            0,
            &handle.arms[0].op.op.fqn,
        )
        .expect("plan-driven direct escape resolution should succeed");
        let b1_id = find_handle_local_id_by_name(handle, "b1").expect("expected local `b1`");

        assert_eq!(resolved.perform_sites.len(), 2);
        assert!(resolved.capture_ids.contains(&b1_id));
    }

    #[test]
    fn escape_arm_capture_locals_include_outer_scope_reads() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Provide {
    fun provide(): Int
}

class Cell(var k: Continuation<Int>?)

fun demo(): Unit {
    val none_k: Continuation<Int>? = None()
    val cell: Cell = Cell(none_k)
    val _: Unit = handle {
        val first: Int = Provide.provide()
        val second: Int = Provide.provide()
        println(first + second)
    } with {
        Provide.provide(), k -> {
            cell.k = Some(k)
        }
    }
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let cell_id = find_fun_local_id_by_name(fun, "cell").expect("expected outer local `cell`");

        assert!(plan.arm_capture_locals(0).contains(&cell_id));
    }

    #[test]
    fn resolve_escape_direct_sites_from_plan_captures_outer_local_used_only_in_nested_handle() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

class Box(val value: Int)

effect Ask {
    fun ask(): Int
}

fun demo(): Unit {
    val box: Box = Box(10)
    val _: Unit = handle {
        val v1: Int = Ask.ask()
        val _: Unit = handle {
            val v2: Int = Ask.ask()
            println(box.value + v2)
        } with {
            Ask.ask(), k -> { () }
        }
    } with {
        Ask.ask(), k -> { () }
    }
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let resolved = MainCodegen::resolve_escape_direct_sites_from_plan(
            handle,
            &plan,
            0,
            &handle.arms[0].op.op.fqn,
        )
        .expect("plan-driven direct escape resolution should succeed");
        let box_id = find_fun_local_id_by_name(fun, "box").expect("expected outer local `box`");

        assert!(resolved.capture_ids.contains(&box_id));
    }

    #[test]
    fn resolve_mixed_escape_direct_sites_from_plan_keeps_source_order_and_arm_ids() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect First {
    fun first(): Int
}

effect Second {
    fun second(): Int
}

fun demo(): Int {
    val result: Int = handle {
        val a: Int = Second.second()
        val b: Int = First.first()
        a + b
    } with {
        First.first(), k -> 10
        Second.second(), k -> 20
    }
    result
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let escape_arms = handle
            .arms
            .iter()
            .enumerate()
            .map(|(idx, arm)| (arm, idx as u32))
            .collect::<Vec<_>>();

        let resolved = MainCodegen::resolve_mixed_escape_direct_sites_from_plan(
            handle,
            &plan,
            escape_arms.as_slice(),
        )
        .expect("plan-driven mixed direct escape resolution should succeed");

        assert_eq!(resolved.direct_sites.len(), 2);
        assert_eq!(resolved.direct_sites[0].arm_id, 1);
        assert_eq!(resolved.direct_sites[1].arm_id, 0);
        assert_eq!(resolved.direct_sites[0].site.top_level_stmt_idx, 0);
        assert_eq!(resolved.direct_sites[1].site.top_level_stmt_idx, 1);
        assert!(resolved
            .direct_sites
            .iter()
            .all(|resolved| resolved.site.resume_path.is_empty()));
    }

    #[test]
    fn resolve_mixed_escape_indirect_sites_from_plan_recovers_nested_paths_and_captures() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        val base: Int = 1
        if (flag) {
            val value: Int = fetch(base)
            value
        } else {
            0
        }
    } with {
        Ask.ask(seed: Int), k -> seed
    }
    result
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let base_id = find_handle_local_id_by_name(handle, "base").expect("expected local `base`");

        let resolved = MainCodegen::resolve_mixed_escape_indirect_sites_from_plan(handle, &plan)
            .expect("plan-driven mixed indirect escape resolution should succeed");

        assert_eq!(resolved.indirect_sites.len(), 1);
        assert!(matches!(
            resolved.indirect_sites[0].resume_path.as_slice(),
            [MixedEscapeDirectFrame::IfThen { .. }]
        ));
        assert!(resolved.capture_ids.contains(&base_id));
    }

    #[test]
    fn mixed_representative_sample_keeps_full_plan_and_simplification_in_sync() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        val first: Int = Yield.next()
        if (flag) {
            val second: Int = Ask.ask(first)
            first + second
        } else {
            first
        }
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Ask.ask(seed: Int), k -> seed + 2
    }
    result
}
"#;
        let plan_dump = build_plan_dump(source);
        let simplification_dump = build_mode_specific_dump(source);

        assert!(plan_dump.contains("mode=immediate-resume"));
        assert!(plan_dump.contains("mode=escape-continuation"));
        assert!(simplification_dump.contains("lowering=stack-reenter"));
        assert!(simplification_dump.contains("lowering=heap-continuation"));
        assert!(simplification_dump.contains("stack-reentry=yes"));
        assert!(simplification_dump.contains("heap-continuation=yes"));
    }

    #[test]
    fn simplification_codegen_entrypoint_classifies_single_modes() {
        let never_resume = build_codegen_entrypoint_label(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Ask.ask(1)
        x + 1
    } with {
        Ask.ask(seed: Int) -> seed + 10
    }
    result
}
"#,
        );
        let immediate_resume = build_codegen_entrypoint_label(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Ask.ask(1)
        x + 1
    } with {
        Ask.ask(seed: Int) -> resume {
            resume(seed + 10)
        }
    }
    result
}
"#,
        );
        let escape_continuation = build_codegen_entrypoint_label(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Ask.ask(1)
        x + 1
    } with {
        Ask.ask(seed: Int), k -> seed + 10
    }
    result
}
"#,
        );

        assert_eq!(never_resume, "single-nonresuming");
        assert_eq!(immediate_resume, "single-immediate-resume");
        assert_eq!(escape_continuation, "single-escape-continuation");
    }

    #[test]
    fn unified_no_continuation_entrypoint_marks_nosuspend_finally_nested_handle_sample() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        val nested: Int = handle {
            var value: Int = 1
            if (flag) {
                value = value + 2
            } else {
                value = value + 4
            }
            while (value < 6) {
                value = value + 1
            }
            value
        } with {
            Yield.next() -> 99
        } finally {
            println("inner finally")
        }
        if (flag) {
            nested + 10
        } else {
            nested + 20
        }
    } with {
        Yield.next() -> 0
    } finally {
        println("outer finally")
    }
    result
}
"#;

        assert_eq!(build_codegen_entrypoint_label(source), "no-suspend-sites");
        assert_eq!(
            build_unified_no_continuation_entrypoint_label(source),
            Some("no-suspend-sites")
        );
    }

    #[test]
    fn unified_no_continuation_entrypoint_marks_single_nonresuming_sample() {
        let source = r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Ask.ask(1)
        x + 1
    } with {
        Ask.ask(seed: Int) -> seed + 10
    }
    result
}
"#;

        assert_eq!(build_codegen_entrypoint_label(source), "single-nonresuming");
        assert_eq!(
            build_unified_no_continuation_entrypoint_label(source),
            Some("single-nonresuming")
        );
    }

    #[test]
    fn unified_no_continuation_entrypoint_marks_single_nonresuming_finally_nested_handle_sample() {
        let source = r#"
package a

import scoop.core.*

effect Alarm {
    fun trip(code: Int): Nothing
}

fun demo(): Int {
    val result: Int = handle {
        val inner: Int = handle {
            Raise.raise(7)
            0
        } with {
            Raise.raise(err: Int) -> err + 1
        } finally {
            println("inner finally")
        }
        Alarm.trip(inner + 1)
        0
    } with {
        Alarm.trip(code: Int) -> code + 10
    } finally {
        println("outer finally")
    }
    result
}
"#;

        assert_eq!(build_codegen_entrypoint_label(source), "single-nonresuming");
        assert_eq!(
            build_unified_no_continuation_entrypoint_label(source),
            Some("single-nonresuming")
        );
    }

    #[test]
    fn unified_no_continuation_entrypoint_marks_multi_nonresuming_finally_nested_handle_sample() {
        let source = r#"
package a

import scoop.core.*

effect Alarm {
    fun trip(code: Int): Nothing
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        val inner: Int = handle {
            Raise.raise(4)
            0
        } with {
            Raise.raise(err: Int) -> err + 1
        } finally {
            println("inner finally")
        }

        if (flag) {
            Alarm.trip(inner + 1)
            0
        } else {
            Raise.raise(inner + 2)
            0
        }
    } with {
        Alarm.trip(code: Int) -> code + 10
        Raise.raise(err: Int) -> err + 20
    } finally {
        println("outer finally")
    }
    result
}
"#;

        assert_eq!(build_codegen_entrypoint_label(source), "multi-nonresuming");
        assert_eq!(
            build_unified_no_continuation_entrypoint_label(source),
            Some("multi-nonresuming")
        );
    }

    #[test]
    fn unified_single_resuming_entrypoint_marks_single_immediate_resume_while_nested_handle_sample()
    {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Alarm {
    fun trip(code: Int): Nothing
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        val seed: Int = handle {
            if (flag) {
                1
            } else {
                2
            }
        } with {
            Alarm.trip(code: Int) -> code + 40
        } finally {
            println("inner finally")
        }

        var total: Int = seed
        while (total < 5) {
            val step: Int = Yield.next()
            total = total + step
        }
        total
    } with {
        Yield.next() -> resume {
            println("resume")
            resume(1)
        }
    } finally {
        println("outer finally")
    }
    result
}
"#;

        assert_eq!(build_codegen_entrypoint_label(source), "single-immediate-resume");
        assert_eq!(
            build_unified_single_resuming_entrypoint_label(source),
            Some("single-immediate-resume")
        );
    }

    #[test]
    fn unified_single_resuming_entrypoint_marks_single_escape_direct_if_nested_handle_sample() {
        let source = r#"
package a

import scoop.core.*

effect Ask {
    fun get(seed: Int): Int
}

effect Alarm {
    fun trip(code: Int): Nothing
}

fun demo(flag: Bool): Unit {
    var saved: Continuation<Int>? = None()

    val handled: Unit = handle {
        val base: Int = handle {
            if (flag) {
                1
            } else {
                2
            }
        } with {
            Alarm.trip(code: Int) -> code + 30
        } finally {
            println("inner finally")
        }

        if (flag) {
            val value: Int = Ask.get(base)
            println(value)
        }
    } with {
        Ask.get(seed), k -> {
            saved = Some(k)
        }
    } finally {
        println("outer finally")
    }

    val keep_saved: Continuation<Int>? = saved
    val _: Continuation<Int>? = keep_saved
}
"#;

        assert_eq!(build_codegen_entrypoint_label(source), "single-escape-continuation");
        assert_eq!(
            build_unified_single_resuming_entrypoint_label(source),
            Some("single-escape-continuation")
        );
    }

    #[test]
    fn unified_single_resuming_entrypoint_marks_single_escape_indirect_nested_handle_sample() {
        let source = r#"
package a

import scoop.core.*

effect Ask {
    fun get(seed: Int): Int
}

effect Alarm {
    fun trip(code: Int): Nothing
}

fun fetch(seed: Int): Int / (Ask) {
    val value: Int = Ask.get(seed + 1)
    value + 2
}

fun demo(flag: Bool): Unit {
    var saved: Continuation<Int>? = None()

    val handled: Unit = handle {
        val base: Int = handle {
            if (flag) {
                10
            } else {
                20
            }
        } with {
            Alarm.trip(code: Int) -> code + 5
        } finally {
            println("inner finally")
        }

        val value: Int = fetch(base)
        println(value)
    } with {
        Ask.get(seed), k -> {
            saved = Some(k)
        }
    } finally {
        println("outer finally")
    }

    val keep_saved: Continuation<Int>? = saved
    val _: Continuation<Int>? = keep_saved
}
"#;

        assert_eq!(build_codegen_entrypoint_label(source), "single-escape-continuation");
        assert_eq!(
            build_unified_single_resuming_entrypoint_label(source),
            Some("single-escape-continuation")
        );
    }

    #[test]
    fn unified_multi_resuming_entrypoint_marks_multiple_immediate_top_level_sample() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Step {
    fun take(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val first: Int = Yield.next()
        val second: Int = Step.take(first + 1)
        first + second
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Step.take(seed: Int) -> resume {
            resume(seed * 2)
        }
    }
    result
}
"#;

        assert_eq!(
            build_unified_multi_resuming_entrypoint_label(source),
            Some("multiple-immediate-top-level")
        );
    }

    #[test]
    fn unified_multi_resuming_entrypoint_marks_multiple_escape_top_level_direct_sample() {
        let source = r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

effect Count {
    fun next(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val first: Int = Ask.ask(1)
        val second: Int = Count.next(first + 1)
        first + second
    } with {
        Ask.ask(seed: Int), k -> seed + 10
        Count.next(seed: Int), k -> seed + 20
    }
    result
}
"#;

        assert_eq!(
            build_unified_multi_resuming_entrypoint_label(source),
            Some("multiple-escape-top-level-direct")
        );
    }

    #[test]
    fn unified_multi_resuming_entrypoint_marks_immediate_with_nonresuming_sample() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Abort {
    fun stop(seed: Int): Nothing
}

fun demo(): Int {
    val result: Int = handle {
        val first: Int = Yield.next()
        if (first > 0) {
            Abort.stop(first)
        }
        first
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Abort.stop(seed: Int) -> 0
    }
    result
}
"#;

        assert_eq!(
            build_unified_multi_resuming_entrypoint_label(source),
            Some("immediate-with-nonresuming")
        );
    }

    #[test]
    fn unified_multi_resuming_entrypoint_marks_escape_with_nonresuming_sample() {
        let source = r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

effect Abort {
    fun stop(seed: Int): Nothing
}

fun demo(): Int {
    val result: Int = handle {
        val first: Int = Ask.ask(1)
        if (first > 0) {
            Abort.stop(first)
        }
        first
    } with {
        Ask.ask(seed: Int), k -> seed + 1
        Abort.stop(seed: Int) -> 0
    }
    result
}
"#;

        assert_eq!(
            build_unified_multi_resuming_entrypoint_label(source),
            Some("escape-with-nonresuming")
        );
    }

    #[test]
    fn unified_multi_resuming_entrypoint_marks_immediate_with_escape_sample() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val first: Int = Yield.next()
        val second: Int = Ask.ask(first)
        first + second
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Ask.ask(seed: Int), k -> seed + 1
    }
    result
}
"#;

        assert_eq!(
            build_unified_multi_resuming_entrypoint_label(source),
            Some("immediate-with-escape")
        );
    }

    #[test]
    fn unified_multi_resuming_entrypoint_marks_immediate_with_escape_and_nonresuming_sample() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun ask(seed: Int): Int
}

effect Abort {
    fun stop(seed: Int): Nothing
}

fun demo(): Int {
    val result: Int = handle {
        val first: Int = Yield.next()
        val second: Int = Ask.ask(first)
        if (second > 0) {
            Abort.stop(second)
        }
        first + second
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Ask.ask(seed: Int), k -> seed + 1
        Abort.stop(seed: Int) -> 0
    }
    result
}
"#;

        assert_eq!(
            build_unified_multi_resuming_entrypoint_label(source),
            Some("immediate-with-escape-and-nonresuming")
        );
    }

    #[test]
    fn resolve_top_level_immediate_resume_sites_from_plan_keeps_source_order() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Step {
    fun take(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val first: Int = Yield.next()
        val second: Int = Step.take(first)
        second
    } with {
        Step.take(seed: Int) -> resume {
            resume(seed + 1)
        }
        Yield.next() -> resume {
            resume(10)
        }
    }
    result
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let immediate_arms = handle
            .arms
            .iter()
            .enumerate()
            .map(|(idx, arm)| (arm, idx as u32))
            .collect::<Vec<_>>();

        let resolved = MainCodegen::resolve_top_level_immediate_resume_sites_from_plan(
            handle,
            &plan,
            immediate_arms.as_slice(),
        )
        .expect("plan-driven multiple immediate resolution should succeed");

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].arm_id, 1);
        assert_eq!(resolved[1].arm_id, 0);
        assert_eq!(resolved[0].site.op.fqn, "a.Yield.next");
        assert_eq!(resolved[1].site.op.fqn, "a.Step.take");
        assert_eq!(resolved[0].site.top_level_stmt_idx, 0);
        assert_eq!(resolved[1].site.top_level_stmt_idx, 1);
        assert!(resolved.iter().all(|site| site.site.resume_path.is_empty()));
    }

    #[test]
    fn resolve_immediate_resume_site_from_plan_accepts_nested_while_path() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var keep: Bool = true
        while (keep) {
            if (flag) {
                val x: Int = Yield.next()
                println(x)
            }
            keep = false
        }
        0
    } with {
        Yield.next() -> resume {
            resume(1)
        }
    }
    result
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);

        let resolved = MainCodegen::resolve_immediate_resume_site_from_plan(
            handle,
            &plan,
            0,
            "a.Yield.next",
        )
        .expect("plan-driven immediate resolution should succeed")
        .expect("expected a direct immediate-resume perform site");

        assert_eq!(resolved.top_level_stmt_idx, 1);
        assert_eq!(resolved.op.fqn, "a.Yield.next");
        assert!(matches!(
            resolved.resume_path.as_slice(),
            [ImmediateResumeFrame::WhileBody { .. }, ImmediateResumeFrame::IfThen { .. }]
        ));
    }

    #[test]
    fn resolve_immediate_resume_with_escape_sites_from_plan_recovers_nested_mixed_matrix() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun ask(seed: Int): Int
}

fun askIndirect(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        val base: Int = Yield.next()
        if (flag) {
            val direct: Int = Ask.ask(base)
            direct
        } else {
            val indirect: Int = askIndirect(base)
            indirect
        }
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Ask.ask(seed: Int), k -> seed + 2
    }
    result
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);

        let resolved = MainCodegen::resolve_immediate_resume_with_escape_sites_from_plan(
            handle,
            &plan,
            &handle.arms[0],
            &handle.arms[1],
        )
        .expect("plan-driven immediate+escape resolution should succeed");

        assert_eq!(resolved.perform_site.top_level_stmt_idx, 0);
        assert_eq!(resolved.perform_site.op.fqn, "a.Yield.next");
        assert_eq!(resolved.direct_sites.len(), 1);
        assert_eq!(resolved.indirect_sites.len(), 1);
        assert_eq!(resolved.direct_sites[0].top_level_stmt_idx, 1);
        assert_eq!(resolved.indirect_sites[0].top_level_stmt_idx, 1);
        assert!(matches!(
            resolved.direct_sites[0].resume_path.as_slice(),
            [MixedEscapeDirectFrame::IfThen { .. }]
        ));
        assert!(matches!(
            resolved.indirect_sites[0].resume_path.as_slice(),
            [MixedEscapeDirectFrame::IfElse { .. }]
        ));
    }

    #[test]
    fn simplification_codegen_entrypoint_classifies_mixed_representative_sample() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        val first: Int = Yield.next()
        if (flag) {
            val second: Int = Ask.ask(first)
            first + second
        } else {
            first
        }
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Ask.ask(seed: Int), k -> seed + 2
    }
    result
}
"#;

        assert_eq!(build_codegen_entrypoint_label(source), "immediate-with-escape");
    }

    #[test]
    fn plan_dump_marks_cast_as_runtime_raise_site() {
        let source = r#"
package a

import scoop.core.*

interface Marker {}

class Impl() : Marker

class Other()

fun demo(value: Any): Int {
    val result: Int = try {
        val _other: Other = value as Other
        1
    } catch (e: RuntimeError) {
        0
    }
    result
}
"#;

        let dump = build_plan_dump(source);
        assert!(dump.contains("kind=runtime-raise"));
        assert!(dump.contains("detail=ClassCastFailed"));
        assert_eq!(build_codegen_entrypoint_label(source), "single-nonresuming");
    }

    #[test]
    fn plan_dump_marks_class_ctor_init_as_hidden_suspend_site() {
        let source = r#"
package a

import scoop.core.*

class Boom() {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }
}

fun demo(): Int {
    val result: Int = try {
        val _boom: Boom = Boom()
        1
    } catch (e: RuntimeError) {
        0
    }
    result
}
"#;

        let dump = build_plan_dump(source);
        assert!(dump.contains("kind=class-ctor-init"));
        assert!(dump.contains("detail=a.Boom"));
        assert_eq!(build_codegen_entrypoint_label(source), "single-nonresuming");
    }

    #[test]
    fn plan_dump_marks_object_init_access_as_hidden_suspend_site() {
        let source = r#"
package a

import scoop.core.*

object BoomObject {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }

    val x: Int = 1
}

fun demo(): Int {
    val result: Int = try {
        BoomObject.x
    } catch (e: RuntimeError) {
        0
    }
    result
}
"#;

        let dump = build_plan_dump(source);
        assert!(dump.contains("kind=object-init-access"));
        assert!(dump.contains("detail=a.BoomObject.x"));
        assert_eq!(build_codegen_entrypoint_label(source), "single-nonresuming");
    }

    #[test]
    fn plan_dump_marks_nested_handle_boundary_when_inner_handle_may_suspend() {
        let source = r#"
package a

import scoop.core.*

effect Boom {
    fun boom(code: Int): Nothing
}

fun demo(): Int {
    val result: Int = handle {
        val inner: Int = handle {
            Boom.boom(1)
            11
        } with {
            Boom.boom(code: Int) -> 22
        }
        inner
    } with {
        Boom.boom(code: Int) -> 33
    }
    result
}
"#;

        let dump = build_plan_dump(source);
        assert!(dump.contains("kind=nested-handle-boundary"));
        assert!(dump.contains("detail=nested#0"));
        assert_eq!(build_codegen_entrypoint_label(source), "single-nonresuming");
    }

    #[test]
    fn plan_dump_marks_continuation_resume_as_runtime_raise_site() {
        let source = r#"
package a

import scoop.core.*

fun demo(k: Continuation<Int>): Int {
    val result: Int = try {
        k.resume(1)
        11
    } catch (e: RuntimeError) {
        22
    }
    result
}
"#;

        let dump = build_plan_dump(source);
        assert!(dump.contains("kind=runtime-raise"), "{dump}");
        assert!(dump.contains("detail=Continuation.resume"), "{dump}");
        assert_eq!(build_codegen_entrypoint_label(source), "single-nonresuming");
    }

    #[test]
    fn segment_dump_covers_direct_branch_loop_and_finally() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var sum: Int = 0
        if (flag) {
            val x: Int = Yield.next()
            sum = x
        } else {
            sum = 1
        }
        while (sum < 3) {
            sum = sum + 1
        }
        sum
    } with {
        Yield.next() -> resume {
            resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#,
        );

        assert!(dump.contains("handle-segments span="), "{dump}");
        assert!(dump.contains("cleanup0 kind=finally"), "{dump}");
        assert!(dump.contains("site0 kind=direct-perform"), "{dump}");
        assert!(dump.contains("branch-then"), "{dump}");
        assert!(dump.contains("branch-else"), "{dump}");
        assert!(dump.contains("suspend-resume"), "{dump}");
        assert!(dump.contains("loop re-entry -> s"), "{dump}");
    }

    #[test]
    fn segment_dump_distinguishes_state_machine_callee_and_indirect_call_sites() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(thunk: () -> Int / (Ask)): Int {
    val result: Int = handle {
        val a: Int = fetch(1)
        val b: Int = thunk()
        a + b
    } with {
        Ask.ask(seed) -> resume {
            resume(seed + 10)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-state-machine-callee"), "{dump}");
        assert!(dump.contains("detail=a.fetch"), "{dump}");
        assert!(dump.contains("kind=indirect-call-may-suspend"), "{dump}");
        assert!(dump.contains("path=top[0]"), "{dump}");
        assert!(dump.contains("path=top[1]"), "{dump}");
    }

    #[test]
    fn segment_dump_records_while_body_source_path() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    val result: Int = handle {
        var sum: Int = 0
        while (sum == 0) {
            val x: Int = Yield.next()
            sum = x
        }
        sum
    } with {
        Yield.next() -> resume {
            resume(7)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=direct-perform"), "{dump}");
        assert!(dump.contains("path=top[1] -> while-body[0]"), "{dump}");
    }

    #[test]
    fn segment_dump_records_nested_while_source_path() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(limit: Int): Int {
    val result: Int = handle {
        var outer: Int = 0
        while (outer < limit) {
            var inner: Int = 0
            while (inner < 1) {
                val x: Int = Yield.next()
                inner = inner + x
            }
            outer = outer + 1
        }
        outer
    } with {
        Yield.next() -> resume {
            resume(1)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=direct-perform"), "{dump}");
        assert!(
            dump.contains("path=top[1] -> while-body[1] -> while-body[0]"),
            "{dump}"
        );
        assert!(dump.contains("loop re-entry -> s"), "{dump}");
    }

    #[test]
    fn segment_dump_recurses_nested_handle_boundaries() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Nothing
}

fun demo(mode: Int): Int {
    val result: Int = handle {
        val inner: Int = handle {
            val x: Int = Yield.next()
            x + mode
        } with {
            Yield.next() -> resume {
                resume(10)
            }
        }
        if (mode == 0) {
            val y: Int = Ask.current()
            inner + y
        } else {
            Boom.boom(mode)
            0
        }
    } with {
        Ask.current(), k -> 7
        Boom.boom(code: Int) -> 0
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=nested-handle-boundary"), "{dump}");
        assert!(dump.contains("nested-handles:\n  nested#0"), "{dump}");
        assert!(dump.contains("site0 kind=direct-perform"), "{dump}");
        assert!(
            dump.contains("dispatch:\n  a.Ask.current => [arm0(entry=seg"),
            "{dump}"
        );
        assert!(dump.contains("a.Boom.boom => [arm1(entry=seg"), "{dump}");
        assert!(
            dump.contains("arm-bodies:\n  arm0 op=a.Ask.current mode=escape-continuation"),
            "{dump}"
        );
        assert!(dump.contains("arm1 op=a.Boom.boom mode=never-resume"), "{dump}");
        assert!(
            dump.contains("context=arm-body arm0 mode=escape-continuation"),
            "{dump}"
        );
        assert!(dump.contains("context=arm-body arm1 mode=never-resume"), "{dump}");
    }

    #[test]
    fn segment_dump_records_mixed_arm_cleanup_context_and_dispatch_context() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Log {
    fun current(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Yield.next()
        val y: Int = Log.current(x)
        x + y
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Log.current(seed: Int) -> seed + 1
    } finally {
        println("cleanup")
    }
    result
}
"#,
        );

        assert!(
            dump.contains("dispatch:\n  a.Log.current => [arm1(entry=seg"),
            "{dump}"
        );
        assert!(dump.contains("a.Yield.next => [arm0(entry=seg"), "{dump}");
        assert!(dump.contains("arm0 op=a.Yield.next mode=immediate-resume"), "{dump}");
        assert!(dump.contains("arm1 op=a.Log.current mode=never-resume"), "{dump}");
        assert!(
            dump.contains("context=arm-body arm0 mode=immediate-resume"),
            "{dump}"
        );
        assert!(dump.contains("context=arm-body arm1 mode=never-resume"), "{dump}");
        assert!(dump.contains("context=cleanup-body cleanup0 kind=finally"), "{dump}");
        assert!(dump.contains("cleanup-stack=[cleanup0]"), "{dump}");
    }

    #[test]
    fn segment_dump_covers_richer_mixed_while_direct_and_indirect_sites() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(limit: Int, thunk: (Int) -> Int / (Ask)): Int {
    val result: Int = handle {
        val base: Int = Yield.next()
        var i: Int = 0
        while (i < limit) {
            val direct: Int = Ask.ask(base + i)
            val indirect: Int = thunk(direct)
            println(indirect)
            i = i + 1
        }
        base + i
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Ask.ask(seed: Int), k -> seed + 2
    }
    result
}
"#,
        );

        assert!(
            dump.contains("dispatch:\n  a.Ask.ask => [arm1(entry=seg"),
            "{dump}"
        );
        assert!(dump.contains("a.Yield.next => [arm0(entry=seg"), "{dump}");
        assert!(dump.contains("arm0 op=a.Yield.next mode=immediate-resume"), "{dump}");
        assert!(dump.contains("arm1 op=a.Ask.ask mode=escape-continuation"), "{dump}");
        assert!(dump.contains("kind=direct-perform"), "{dump}");
        assert!(dump.contains("kind=indirect-call-may-suspend"), "{dump}");
        assert!(dump.contains("path=top[2] -> while-body[0]"), "{dump}");
        assert!(dump.contains("path=top[2] -> while-body[1]"), "{dump}");
    }

    #[test]
    fn segment_dump_records_frame_slot_metadata_for_outer_locals_binders_and_nested_handles() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(seed: Int): Int
}

effect Ask {
    fun current(): Int
}

fun demo(seed: Int): Int {
    val base: Int = seed + 1
    val result: Int = handle {
        val local: Int = base + 1
        val inner: Int = handle {
            val asked: Int = Ask.current()
            asked + local + seed
        } with {
            Ask.current() -> resume {
                resume(base)
            }
        }
        val x: Int = Yield.next(local)
        x + inner + local
    } with {
        Yield.next(arg: Int) -> resume {
            resume(arg + base)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("frame-slots:"), "{dump}");
        assert!(dump.contains("base#"), "{dump}");
        assert!(dump.contains("local#"), "{dump}");
        assert!(dump.contains("arg#"), "{dump}");
        assert!(dump.contains("owner=handle-body"), "{dump}");
        assert!(dump.contains("owner=arm0"), "{dump}");
        assert!(dump.contains("lifted=yes"), "{dump}");
        assert!(dump.contains("nested#0"), "{dump}");
    }

    #[test]
    fn segment_builder_contract_rejects_missing_lifted_local_metadata() {
        let mut segment_list = build_segment_list(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(seed: Int): Int
}

fun demo(seed: Int): Int {
    val base: Int = seed + 1
    val result: Int = handle {
        val local: Int = base + 1
        val x: Int = Yield.next(local)
        x + local
    } with {
        Yield.next(arg: Int) -> resume {
            resume(arg + base)
        }
    }
    result
}
"#,
        );
        let base_id = segment_slot_id_named(&segment_list, "base");
        segment_list.lifted_locals.retain(|id| *id != base_id);

        let err = segment_list
            .validate_builder_contract()
            .expect_err("missing lifted-local metadata should fail");
        assert!(err.contains("lifted_locals[] is missing"), "{err}");
        assert!(err.contains("base#"), "{err}");
    }

    #[test]
    fn segment_builder_contract_rejects_dangling_capture_local_ref() {
        let mut segment_list = build_segment_list(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(seed: Int): Int
}

fun demo(seed: Int): Int {
    val base: Int = seed + 1
    val result: Int = handle {
        val local: Int = base + 1
        val x: Int = Yield.next(local)
        x + local
    } with {
        Yield.next(arg: Int) -> resume {
            resume(arg + base)
        }
    }
    result
}
"#,
        );
        let base_id = segment_slot_id_named(&segment_list, "base");
        segment_list.lifted_locals.retain(|id| *id != base_id);
        segment_list.frame_slots.retain(|slot| slot.id != base_id);

        let err = segment_list
            .validate_builder_contract()
            .expect_err("dangling capture local reference should fail");
        assert!(
            err.contains("arm0 capture metadata references missing slot"),
            "{err}"
        );
    }

    #[test]
    fn plan_round_trip_from_segments_preserves_direct_branch_loop_finally_dump() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var sum: Int = 0
        if (flag) {
            val x: Int = Yield.next()
            sum = x
        } else {
            sum = 1
        }
        while (sum < 3) {
            sum = sum + 1
        }
        sum
    } with {
        Yield.next() -> resume {
            resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#;

        assert_eq!(build_round_tripped_plan_dump(source), build_plan_dump(source));
        assert_eq!(
            build_round_tripped_mode_specific_dump(source),
            build_mode_specific_dump(source)
        );
        assert_eq!(
            build_round_tripped_codegen_entrypoint_label(source),
            build_codegen_entrypoint_label(source)
        );
    }

    #[test]
    fn plan_round_trip_from_segments_preserves_indirect_suspend_dump() {
        let source = r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(thunk: () -> Int / (Ask)): Int {
    val result: Int = handle {
        val a: Int = fetch(1)
        val b: Int = thunk()
        a + b
    } with {
        Ask.ask(seed) -> resume {
            resume(seed + 10)
        }
    }
    result
}
"#;

        assert_eq!(build_round_tripped_plan_dump(source), build_plan_dump(source));
        assert_eq!(
            build_round_tripped_mode_specific_dump(source),
            build_mode_specific_dump(source)
        );
    }

    #[test]
    fn plan_round_trip_from_segments_preserves_nested_handle_multi_arm_dump() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Nothing
}

fun demo(mode: Int): Int {
    val result: Int = handle {
        val inner: Int = handle {
            val x: Int = Yield.next()
            x + mode
        } with {
            Yield.next() -> resume {
                resume(10)
            }
        }
        if (mode == 0) {
            val y: Int = Ask.current()
            inner + y
        } else {
            Boom.boom(mode)
            0
        }
    } with {
        Ask.current(), k -> 7
        Boom.boom(code: Int) -> 0
    }
    result
}
"#;

        assert_eq!(build_round_tripped_plan_dump(source), build_plan_dump(source));
        assert_eq!(
            build_round_tripped_mode_specific_dump(source),
            build_mode_specific_dump(source)
        );
        assert_eq!(
            build_round_tripped_codegen_entrypoint_label(source),
            build_codegen_entrypoint_label(source)
        );
    }

    #[test]
    fn segment_round_trip_preserves_typed_emit_ops_and_branch_metadata() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var sum: Int = 0
        if (flag) {
            val x: Int = Yield.next()
            sum = x
        } else {
            sum = 1
        }
        while (sum < 3) {
            sum = sum + 1
        }
        sum
    } with {
        Yield.next() -> resume {
            resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#;

        let source_plan = build_source_plan(source);
        let segment_list = source_plan.build_segment_list();
        segment_list
            .validate_builder_contract()
            .expect("segment builder contract should hold");
        let rebuilt_plan = HandleStateMachinePlan::build_from_segments(&segment_list)
            .expect("segment-only builder should reconstruct full plan");

        assert_eq!(
            collect_plan_exec_signature(&source_plan),
            collect_plan_exec_signature(&rebuilt_plan)
        );

        let while_cond_state = source_plan
            .states
            .iter()
            .find(|state| state.label == "while.cond")
            .expect("expected while.cond state");
        assert!(matches!(
            while_cond_state.actions.first(),
            Some(&HandleStateOp::WhileCondHeader { .. })
        ));
        assert!(matches!(
            &while_cond_state.terminator,
            StateTerminator::Branch {
                condition: HandleBranchCondition::WhileCond { .. },
                ..
            }
        ));

        let has_if_branch_segment = segment_list.segments.iter().any(|segment| {
            matches!(
                &segment.terminator,
                HandleSegmentTerminator::Branch {
                    condition: HandleBranchCondition::IfCond { .. },
                    ..
                }
            )
        });
        assert!(has_if_branch_segment, "expected an if-branch segment terminator");

        let has_bind_local = segment_list
            .segments
            .iter()
            .flat_map(|segment| segment.ops.iter())
            .any(|op| matches!(op, &HandleStateOp::BindLocal { .. }));
        assert!(has_bind_local, "expected typed bind-local op in segment list");
    }

    fn build_plan_dump(source_text: &str) -> String {
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
            .pretty_dump(&lowered.types)
    }

    fn build_segment_list(source_text: &str) -> HandleSegmentList {
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
            .build_segment_list()
    }

    fn build_segment_dump(source_text: &str) -> String {
        let lowered = lower_typed_single_source(source_text);
        let segment_list = build_segment_list(source_text);
        segment_list
            .validate_builder_contract()
            .expect("segment builder contract should hold");
        segment_list.pretty_dump(&lowered.types)
    }

    fn build_round_tripped_plan_dump(source_text: &str) -> String {
        let lowered = lower_typed_single_source(source_text);
        let plan = build_round_tripped_plan(source_text);
        plan.pretty_dump(&lowered.types)
    }

    fn build_mode_specific_dump(source_text: &str) -> String {
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
            .build_mode_specific_simplification()
            .pretty_dump()
    }

    fn build_round_tripped_mode_specific_dump(source_text: &str) -> String {
        build_round_tripped_plan(source_text)
            .build_mode_specific_simplification()
            .pretty_dump()
    }

    fn build_codegen_entrypoint_label(source_text: &str) -> &'static str {
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
            .build_mode_specific_simplification()
            .codegen_entrypoint()
            .label()
    }

    fn build_round_tripped_codegen_entrypoint_label(source_text: &str) -> &'static str {
        build_round_tripped_plan(source_text)
            .build_mode_specific_simplification()
            .codegen_entrypoint()
            .label()
    }

    fn build_unified_no_continuation_entrypoint_label(
        source_text: &str,
    ) -> Option<&'static str> {
        let plan = build_source_plan(source_text);
        MainCodegen::unified_no_continuation_entrypoint_label_for_plan(&plan)
    }

    fn build_unified_single_resuming_entrypoint_label(
        source_text: &str,
    ) -> Option<&'static str> {
        let plan = build_source_plan(source_text);
        MainCodegen::unified_single_resuming_entrypoint_label_for_plan(&plan)
    }

    fn build_unified_multi_resuming_entrypoint_label(
        source_text: &str,
    ) -> Option<&'static str> {
        let plan = build_source_plan(source_text);
        MainCodegen::unified_multi_resuming_entrypoint_label_for_plan(&plan)
    }

    fn build_source_plan(source_text: &str) -> HandleStateMachinePlan {
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
    }

    fn build_round_tripped_plan(source_text: &str) -> HandleStateMachinePlan {
        let source_plan = build_source_plan(source_text);
        let segment_list = source_plan.build_segment_list();
        segment_list
            .validate_builder_contract()
            .expect("segment builder contract should hold");
        HandleStateMachinePlan::build_from_segments(&segment_list)
            .expect("segment-only builder should reconstruct full plan")
    }

    fn collect_plan_exec_signature(
        plan: &HandleStateMachinePlan,
    ) -> Vec<(String, Vec<usize>, Option<usize>)> {
        plan.states
            .iter()
            .map(|state| {
                let branch_sig = match &state.terminator {
                    StateTerminator::Branch { condition, .. } => {
                        Some(condition.structural_signature())
                    }
                    _ => None,
                };
                (
                    state.label.clone(),
                    state
                        .actions
                        .iter()
                        .map(HandleStateOp::structural_signature)
                        .collect(),
                    branch_sig,
                )
            })
            .collect()
    }

    fn segment_slot_id_named(segment_list: &HandleSegmentList, name: &str) -> hir::SymbolId {
        segment_list
            .frame_slots
            .iter()
            .find(|slot| slot.name == name)
            .map(|slot| slot.id)
            .unwrap_or_else(|| panic!("expected frame slot named {name}"))
    }

    fn lower_typed_single_source(source_text: &str) -> hir::LoweredHir {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual("<mem>", source_text);
        let mut ast = parse_file(&source).unwrap();

        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).unwrap()
        };

        let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));

        hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &[(&source, &ast)],
            &[],
            &typecheck_types,
        )
        .unwrap()
    }

    fn first_handle_in_file(file: &hir::File) -> Option<(&hir::FunDecl, &hir::HandleExpr)> {
        for item in &file.items {
            if let hir::Item::Fun(fun) = item
                && let Some(body) = &fun.body
                && let Some(handle) = first_handle_in_block(body)
            {
                return Some((fun, handle));
            }
        }
        None
    }

    fn first_handle_in_block(block: &hir::Block) -> Option<&hir::HandleExpr> {
        for stmt in &block.stmts {
            if let Some(handle) = first_handle_in_stmt(stmt) {
                return Some(handle);
            }
        }
        None
    }

    fn first_handle_in_stmt(stmt: &hir::Stmt) -> Option<&hir::HandleExpr> {
        match &stmt.kind {
            hir::StmtKind::Expr(expr) => first_handle_in_expr(expr),
            hir::StmtKind::Val(decl) => decl.init.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::StmtKind::While { cond, body } => {
                first_handle_in_expr(cond).or_else(|| first_handle_in_block(body))
            }
            hir::StmtKind::Return { value } => value.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
        }
    }

    fn first_handle_in_expr(expr: &hir::Expr) -> Option<&hir::HandleExpr> {
        match &expr.kind {
            hir::ExprKind::Handle(handle) => Some(handle),
            hir::ExprKind::Block(block) => first_handle_in_block(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => first_handle_in_expr(cond)
                .or_else(|| first_handle_in_expr(then_branch))
                .or_else(|| else_branch.as_deref().and_then(first_handle_in_expr)),
            hir::ExprKind::Call { callee, args } => first_handle_in_expr(callee).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                    hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
                })
            }),
            hir::ExprKind::StructLit { fields, .. } => {
                fields.iter().find_map(|field| first_handle_in_expr(&field.value))
            }
            hir::ExprKind::TupleLit { elements } => elements.iter().find_map(first_handle_in_expr),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    first_handle_in_expr(expr)
                } else {
                    None
                }
            }),
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => first_handle_in_expr(inner),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::ExprKind::When { subject, arms } => first_handle_in_expr(subject).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(first_handle_in_expr)
                        .or_else(|| first_handle_in_expr(&arm.body))
                })
            }),
            hir::ExprKind::Closure(closure) => first_handle_in_expr(&closure.body),
            hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
            }),
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => None,
        }
    }

    fn collect_plan_context(
        lowered: &hir::LoweredHir,
        owner_fun: &hir::FunDecl,
    ) -> HandlePlanContext {
        let mut known_fun_effects = HashMap::new();
        for item in &lowered.file.items {
            if let hir::Item::Fun(fun) = item {
                known_fun_effects.insert(
                    fun.fqn.clone(),
                    fun_effects_are_non_pure(&lowered.types, fun.ty),
                );
            }
        }
        for fun in &lowered.member_funs {
            known_fun_effects.insert(
                fun.fqn.clone(),
                fun_effects_are_non_pure(&lowered.types, fun.ty),
            );
        }

        let mut known_local_fun_effects = HashMap::new();
        for param in &owner_fun.params {
            known_local_fun_effects.insert(
                param.id,
                fun_effects_are_non_pure(&lowered.types, param.ty),
            );
        }
        if let Some(body) = &owner_fun.body {
            collect_local_fun_effects_in_block(body, &lowered.types, &mut known_local_fun_effects);
        }

        let ctor_call_targets = lowered
            .ctor_call_sites
            .iter()
            .map(|(span, targets)| {
                let mut stable_targets = targets.clone();
                stable_targets.sort();
                stable_targets.dedup();
                (*span, stable_targets)
            })
            .collect();
        let object_value_fqns: HashSet<String> = lowered.object_inits.keys().cloned().collect();
        let object_property_fqns: HashSet<String> = lowered
            .object_inits
            .iter()
            .flat_map(|(owner_fqn, object_init)| {
                object_init
                    .properties
                    .keys()
                    .map(|name| format!("{owner_fqn}.{name}"))
                    .collect::<Vec<_>>()
            })
            .collect();

        HandlePlanContext {
            known_fun_effects,
            known_local_fun_effects,
            ctor_call_targets,
            object_value_fqns,
            object_property_fqns,
        }
    }

    fn find_handle_local_id_by_name(handle: &hir::HandleExpr, name: &str) -> Option<hir::SymbolId> {
        fn find_in_stmts(stmts: &[hir::Stmt], name: &str) -> Option<hir::SymbolId> {
            for stmt in stmts {
                match &stmt.kind {
                    hir::StmtKind::Val(decl) => {
                        if decl.name.as_deref() == Some(name) {
                            return decl.id;
                        }
                    }
                    hir::StmtKind::Expr(expr) => {
                        if let Some(id) = find_in_expr(expr, name) {
                            return Some(id);
                        }
                    }
                    hir::StmtKind::While { body, .. } => {
                        if let Some(id) = find_in_stmts(&body.stmts, name) {
                            return Some(id);
                        }
                    }
                    hir::StmtKind::Assign { .. }
                    | hir::StmtKind::Return { .. }
                    | hir::StmtKind::Empty
                    | hir::StmtKind::Break { .. }
                    | hir::StmtKind::Continue { .. }
                    | hir::StmtKind::Todo(_) => {}
                }
            }
            None
        }

        fn find_in_expr(expr: &hir::Expr, name: &str) -> Option<hir::SymbolId> {
            match &expr.kind {
                hir::ExprKind::Block(block) => find_in_stmts(&block.stmts, name),
                hir::ExprKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => find_in_expr(then_branch, name)
                    .or_else(|| else_branch.as_deref().and_then(|expr| find_in_expr(expr, name))),
                hir::ExprKind::When { arms, .. } => arms
                    .iter()
                    .find_map(|arm| find_in_expr(&arm.body, name)),
                _ => None,
            }
        }

        find_in_stmts(&handle.body.stmts, name)
    }

    fn find_fun_local_id_by_name(fun: &hir::FunDecl, name: &str) -> Option<hir::SymbolId> {
        let body = fun.body.as_ref()?;

        fn find_in_stmts(stmts: &[hir::Stmt], name: &str) -> Option<hir::SymbolId> {
            for stmt in stmts {
                match &stmt.kind {
                    hir::StmtKind::Val(decl) => {
                        if decl.name.as_deref() == Some(name) {
                            return decl.id;
                        }
                        if let Some(init) = decl.init.as_ref()
                            && let Some(id) = find_in_expr(init, name)
                        {
                            return Some(id);
                        }
                    }
                    hir::StmtKind::Expr(expr) => {
                        if let Some(id) = find_in_expr(expr, name) {
                            return Some(id);
                        }
                    }
                    hir::StmtKind::Assign { lhs, rhs, .. } => {
                        if let Some(id) = find_in_expr(lhs, name) {
                            return Some(id);
                        }
                        if let Some(id) = find_in_expr(rhs, name) {
                            return Some(id);
                        }
                    }
                    hir::StmtKind::Return { value } => {
                        if let Some(expr) = value
                            && let Some(id) = find_in_expr(expr, name)
                        {
                            return Some(id);
                        }
                    }
                    hir::StmtKind::While { cond, body } => {
                        if let Some(id) = find_in_expr(cond, name) {
                            return Some(id);
                        }
                        if let Some(id) = find_in_stmts(&body.stmts, name) {
                            return Some(id);
                        }
                    }
                    hir::StmtKind::Empty
                    | hir::StmtKind::Break { .. }
                    | hir::StmtKind::Continue { .. }
                    | hir::StmtKind::Todo(_) => {}
                }
            }
            None
        }

        fn find_in_expr(expr: &hir::Expr, name: &str) -> Option<hir::SymbolId> {
            match &expr.kind {
                hir::ExprKind::Block(block) => find_in_stmts(&block.stmts, name),
                hir::ExprKind::If {
                    cond,
                    then_branch,
                    else_branch,
                } => find_in_expr(cond, name)
                    .or_else(|| find_in_expr(then_branch, name))
                    .or_else(|| else_branch.as_deref().and_then(|expr| find_in_expr(expr, name))),
                hir::ExprKind::When { subject, arms } => find_in_expr(subject, name).or_else(|| {
                    arms.iter().find_map(|arm| {
                        arm.guard
                            .as_ref()
                            .and_then(|guard| find_in_expr(guard, name))
                            .or_else(|| find_in_expr(&arm.body, name))
                    })
                }),
                hir::ExprKind::Handle(handle) => find_in_stmts(&handle.body.stmts, name)
                    .or_else(|| {
                        handle
                            .arms
                            .iter()
                            .find_map(|arm| find_in_expr(&arm.body, name))
                    })
                    .or_else(|| {
                        handle
                            .finally
                            .as_ref()
                            .and_then(|block| find_in_stmts(&block.stmts, name))
                    }),
                hir::ExprKind::Call { callee, args } => find_in_expr(callee, name).or_else(|| {
                    args.iter().find_map(|arg| match arg {
                        hir::CallArg::Positional(expr) => find_in_expr(expr, name),
                        hir::CallArg::Named { value, .. } => find_in_expr(value, name),
                    })
                }),
                hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                    hir::CallArg::Positional(expr) => find_in_expr(expr, name),
                    hir::CallArg::Named { value, .. } => find_in_expr(value, name),
                }),
                hir::ExprKind::Binary { lhs, rhs, .. } => {
                    find_in_expr(lhs, name).or_else(|| find_in_expr(rhs, name))
                }
                hir::ExprKind::Unary { expr: inner, .. }
                | hir::ExprKind::Cast { expr: inner, .. }
                | hir::ExprKind::TypeCheck { expr: inner, .. }
                | hir::ExprKind::MemberAccess {
                    receiver: inner, ..
                } => find_in_expr(inner, name),
                hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                    match part {
                        hir::InterpolatedStringPart::Expr { expr } => find_in_expr(expr, name),
                        _ => None,
                    }
                }),
                hir::ExprKind::StructLit { fields, .. } => fields
                    .iter()
                    .find_map(|field| find_in_expr(&field.value, name)),
                hir::ExprKind::TupleLit { elements } => {
                    elements.iter().find_map(|element| find_in_expr(element, name))
                }
                hir::ExprKind::Closure(closure) => find_in_expr(&closure.body, name),
                hir::ExprKind::Missing
                | hir::ExprKind::Literal(_)
                | hir::ExprKind::VarRef(_)
                | hir::ExprKind::UnresolvedIdent { .. }
                | hir::ExprKind::Todo(_) => None,
            }
        }

        find_in_stmts(&body.stmts, name)
    }

    fn fun_effects_are_non_pure(types: &TypeStore, ty: TypeId) -> bool {
        match types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => !fun_ty.effects.is_pure(),
            _ => false,
        }
    }

    fn collect_local_fun_effects_in_block(
        block: &hir::Block,
        types: &TypeStore,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        for stmt in &block.stmts {
            collect_local_fun_effects_in_stmt(stmt, types, out);
        }
    }

    fn collect_local_fun_effects_in_stmt(
        stmt: &hir::Stmt,
        types: &TypeStore,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        match &stmt.kind {
            hir::StmtKind::Val(decl) => {
                if let Some(id) = decl.id {
                    out.insert(id, fun_effects_are_non_pure(types, decl.ty));
                }
                if let Some(init) = decl.init.as_ref() {
                    collect_local_fun_effects_in_expr(init, types, out);
                }
            }
            hir::StmtKind::Expr(expr) => collect_local_fun_effects_in_expr(expr, types, out),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                collect_local_fun_effects_in_expr(lhs, types, out);
                collect_local_fun_effects_in_expr(rhs, types, out);
            }
            hir::StmtKind::While { cond, body } => {
                collect_local_fun_effects_in_expr(cond, types, out);
                collect_local_fun_effects_in_block(body, types, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    collect_local_fun_effects_in_expr(expr, types, out);
                }
            }
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
        }
    }

    fn collect_local_fun_effects_in_expr(
        expr: &hir::Expr,
        types: &TypeStore,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        match &expr.kind {
            hir::ExprKind::Block(block) => collect_local_fun_effects_in_block(block, types, out),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_local_fun_effects_in_expr(cond, types, out);
                collect_local_fun_effects_in_expr(then_branch, types, out);
                if let Some(else_branch) = else_branch.as_deref() {
                    collect_local_fun_effects_in_expr(else_branch, types, out);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                collect_local_fun_effects_in_expr(subject, types, out);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        collect_local_fun_effects_in_expr(guard, types, out);
                    }
                    collect_local_fun_effects_in_expr(&arm.body, types, out);
                }
            }
            hir::ExprKind::Call { callee, args } => {
                collect_local_fun_effects_in_expr(callee, types, out);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            collect_local_fun_effects_in_expr(expr, types, out)
                        }
                        hir::CallArg::Named { value, .. } => {
                            collect_local_fun_effects_in_expr(value, types, out)
                        }
                    }
                }
            }
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    collect_local_fun_effects_in_expr(&field.value, types, out);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for element in elements {
                    collect_local_fun_effects_in_expr(element, types, out);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        collect_local_fun_effects_in_expr(expr, types, out);
                    }
                }
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => collect_local_fun_effects_in_expr(inner, types, out),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                collect_local_fun_effects_in_expr(lhs, types, out);
                collect_local_fun_effects_in_expr(rhs, types, out);
            }
            hir::ExprKind::Closure(closure) => {
                collect_local_fun_effects_in_expr(&closure.body, types, out);
            }
            hir::ExprKind::Handle(handle) => {
                collect_local_fun_effects_in_block(&handle.body, types, out);
                for arm in &handle.arms {
                    for binder in &arm.op.binders {
                        out.insert(binder.id, fun_effects_are_non_pure(types, binder.ty));
                    }
                    collect_local_fun_effects_in_expr(&arm.body, types, out);
                }
                if let Some(finally_block) = &handle.finally {
                    collect_local_fun_effects_in_block(finally_block, types, out);
                }
            }
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            collect_local_fun_effects_in_expr(expr, types, out)
                        }
                        hir::CallArg::Named { value, .. } => {
                            collect_local_fun_effects_in_expr(value, types, out)
                        }
                    }
                }
            }
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => {}
        }
    }
}
