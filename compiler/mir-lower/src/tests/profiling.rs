use super::*;
use scoop_profile::{InlineOrigin, PositionKind};

mod feedback;

fn lower_profile(
    source: &hir::ExportHirOutput,
    input: ProfilingInput<'_>,
    elision: bool,
) -> mir::Module {
    let concrete = scoop_hir_lower::concretize_output(source).unwrap();
    let (mut module, _) = crate::lower_with_dependencies(
        &concrete,
        Arena::new(),
        HashMap::new(),
        Vec::new(),
        &[],
        elision,
        input.enabled(),
    );
    input.apply(&mut module.meta.profiling);
    module
}

fn plain_program(constant: i32, padding: bool) -> hir::ExportHirOutput {
    let mut h = Harness::new();
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", h.int));
    let leaf = h.user_fn_full(
        "leaf",
        vec![],
        vec![param("value", h.int, value)],
        h.int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: local_ref(value, h.int),
            })],
        },
    );
    // This harness derives non-entry declaration identities from their slot.
    // Keep the callee identity fixed while moving the entry's arena slot.
    if padding {
        h.user_fn(
            "unrelated",
            hir::Body {
                locals: Arena::new(),
                statements: vec![],
            },
        );
    }
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call_typed(leaf, vec![int_lit(&h, constant)], h.int)),
                expr_stmt(call_typed(leaf, vec![int_lit(&h, 31)], h.int)),
            ],
        },
    );
    h.finish(main)
}

fn layout<'a>(module: &'a mir::Module, name: &str) -> &'a scoop_profile::BodyLayout {
    module
        .meta
        .profiling
        .plans
        .iter()
        .find(|plan| plan.name == name)
        .unwrap()
}

#[test]
fn profiling_body_keys_ignore_other_function_arena_allocations() {
    let plain = lower_profile(&plain_program(24, false), ProfilingInput::Generate, false);
    let padded = lower_profile(&plain_program(24, true), ProfilingInput::Generate, false);
    for name in ["main", "leaf"] {
        assert_eq!(
            layout(&plain, name).key,
            layout(&padded, name).key,
            "{name}"
        );
        assert_eq!(
            layout(&plain, name).positions,
            layout(&padded, name).positions
        );
    }
    let changed = lower_profile(&plain_program(25, false), ProfilingInput::Generate, false);
    assert_eq!(
        layout(&plain, "main").key.body,
        layout(&changed, "main").key.body
    );
    assert_ne!(
        layout(&plain, "main").key.fingerprint,
        layout(&changed, "main").key.fingerprint
    );
    assert_eq!(layout(&plain, "leaf").key, layout(&changed, "leaf").key);
}

#[test]
fn profiling_retains_logical_entries_and_distinct_inline_origins() {
    let mut module = lower_profile(&plain_program(24, false), ProfilingInput::Generate, false);
    let leaf = layout(&module, "leaf").key;
    let plans = module.meta.profiling.plans.clone();
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    crate::optimization::run(&mut module, &selected, MirOptimizationOptions::RELEASE);
    assert_eq!(plans, module.meta.profiling.plans);
    let main = &module.functions[module.executable_entry().unwrap()];
    let entries = main
        .body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .filter_map(|statement| match &statement.kind {
            mir::StatementKind::ProfileCounter(marker) if marker.kind == PositionKind::Entry => {
                Some(marker.resolve(&module.meta.profiling))
            }
            _ => None,
        })
        .filter(|counter| counter.location.body == leaf)
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0].inlined_at, entries[1].inlined_at);
    assert!(
        entries
            .iter()
            .all(|entry| matches!(entry.inlined_at.as_slice(), [InlineOrigin::Call(_)]))
    );
    assert!(!main.body.blocks.iter().any(|(_, block)| {
        block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, mir::StatementKind::Call(_)))
    }));
}

#[test]
fn profiling_does_not_disable_coroutine_tail_forwarding() {
    let source = super::coroutine_tail::sample::program(
        super::coroutine_tail::sample::Shape::Exact,
        2,
        false,
    );
    let module = lower_profile(&source, ProfilingInput::Generate, true);
    for name in ["wrapper0", "wrapper1"] {
        let coroutine = module
            .meta
            .coroutine_functions
            .iter()
            .find_map(|(_, coroutine)| {
                (module.functions[coroutine.function].name == name).then_some(coroutine)
            })
            .unwrap();
        assert!(
            matches!(coroutine.lowering, mir::CoroutineLowering::TailForwarding),
            "{name}"
        );
    }
}

#[test]
fn profiling_separates_consecutive_yields_and_source_entry_from_resume() {
    let mut h = Harness::new();
    let yielding = h.user_fn(
        "yieldDrive",
        hir::Body {
            locals: Arena::new(),
            statements: vec![],
        },
    );
    h.functions[yielding].is_suspend = true;
    h.functions[yielding].kind = hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
        kind: hir::IntrinsicFunctionKind::CoroutineYield,
        provider: hir::IntrinsicProviderId::from_raw(0),
    });
    let caller = h.user_fn(
        "yieldCaller",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call_typed(yielding, vec![], h.unit)),
                expr_stmt(call_typed(yielding, vec![], h.unit)),
            ],
        },
    );
    h.functions[caller].is_suspend = true;
    let main = empty_main(&mut h);
    let module = lower_profile(&h.finish_coroutines(main), ProfilingInput::Generate, true);
    let source = layout(&module, "yieldCaller");
    assert_eq!(
        source
            .positions
            .iter()
            .filter(|position| position.kind == PositionKind::Call)
            .count(),
        2
    );
    let (_, coroutine) = module
        .meta
        .coroutine_functions
        .iter()
        .find(|(_, coroutine)| module.functions[coroutine.function].name == "yieldCaller")
        .unwrap();
    let mir::CoroutineLowering::StateMachine {
        driver,
        resume_points,
        ..
    } = &coroutine.lowering
    else {
        panic!("yields require a real frame")
    };
    assert_eq!(resume_points.len(), 2);
    let own_entries = |function: &mir::Function| {
        function.body.blocks.iter().flat_map(|(_, block)| &block.statements)
        .filter(|statement| matches!(&statement.kind, mir::StatementKind::ProfileCounter(marker)
            if marker.kind == PositionKind::Entry && marker.resolve(&module.meta.profiling).location.body == source.key)).count()
    };
    assert_eq!(own_entries(&module.functions[coroutine.function]), 1);
    assert_eq!(own_entries(&module.functions[*driver]), 0);
}
