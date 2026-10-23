//! Local optimizations over complete MIR, before output validation and LIR.

use scoop_mir as mir;

mod allocation;
mod constants;
mod dispatch;
mod flow;
mod graph;
mod inline;
mod interface_views;
mod intrinsics;
mod poll_chunks;
mod simplify;

/// Producer choices; they do not change entity or ABI identity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MirOptimizationOptions {
    pub devirtualize: bool,
    pub inline: bool,
    pub allocation_elision: bool,
    pub interface_views: bool,
    pub coroutine_elision: bool,
    pub poll_chunking: bool,
}

impl MirOptimizationOptions {
    pub const RELEASE: Self = Self {
        devirtualize: true,
        inline: true,
        allocation_elision: true,
        interface_views: true,
        coroutine_elision: true,
        poll_chunking: true,
    };
}

pub(super) fn inline_synchronous_coroutines(lowerer: &mut crate::Lowerer) {
    inline::inline_synchronous_coroutines(lowerer);
}

#[cfg(test)]
pub(super) fn run(
    module: &mut mir::Module,
    selected: &mir::SelectedExternalMirSet,
    options: MirOptimizationOptions,
) -> (usize, bool) {
    run_to_limit(module, selected, options, 3)
}

pub(super) fn run_current(
    module: &mut mir::Module,
    selected: &mir::SelectedExternalMirSet,
    source: &scoop_hir::concrete::Module,
    options: MirOptimizationOptions,
    mut imported: crate::imported_bodies::ImportedBodies,
) -> (usize, bool) {
    let required = intrinsics::entries(module, source, std::mem::take(&mut imported.required));
    run_with_intrinsics(module, selected, options, 3, &required, imported)
}

/// Return the rounds used and whether an unchanged round was observed.
#[cfg(test)]
pub(super) fn run_to_limit(
    module: &mut mir::Module,
    selected: &mir::SelectedExternalMirSet,
    options: MirOptimizationOptions,
    limit: usize,
) -> (usize, bool) {
    run_with_intrinsics(
        module,
        selected,
        options,
        limit,
        &Default::default(),
        Default::default(),
    )
}

fn run_with_intrinsics(
    module: &mut mir::Module,
    selected: &mir::SelectedExternalMirSet,
    options: MirOptimizationOptions,
    limit: usize,
    required: &intrinsics::Entries,
    mut imported: crate::imported_bodies::ImportedBodies,
) -> (usize, bool) {
    let mut registry =
        crate::local_values::LocalValueRegistry::from_existing(&module.meta.local_values);
    let dispatch = std::mem::take(&mut imported.dispatch);
    let has_intrinsics = !required.is_empty();
    let mut inliner = (options.inline || has_intrinsics).then(|| {
        inline::Session::new(
            module,
            required.clone(),
            options.inline,
            options.allocation_elision,
            imported,
        )
    });
    if has_intrinsics {
        intrinsics::normalize(
            module,
            selected,
            required,
            &dispatch,
            inliner.as_mut().unwrap(),
            &mut registry,
        );
    }
    if !options.devirtualize
        && !options.inline
        && !options.allocation_elision
        && !options.interface_views
        && !options.poll_chunking
    {
        module.meta.local_values = registry.finish();
        return (0, true);
    }
    let mut active = vec![true; module.functions.len()];
    let mut outcome = (limit, false);
    for round in 0..limit {
        let mut changed = if options.devirtualize {
            devirtualize(module, selected, &active, None, &dispatch)
        } else {
            vec![false; active.len()]
        };
        if options.inline
            && let Some(inliner) = &mut inliner
        {
            for (changed, inlined) in
                changed
                    .iter_mut()
                    .zip(inliner.run(module, &active, &mut registry))
            {
                *changed |= inlined;
            }
        }
        if options.allocation_elision {
            for (changed, elided) in
                changed
                    .iter_mut()
                    .zip(allocation::run(module, &active, &mut registry))
            {
                *changed |= elided;
            }
        }
        if has_intrinsics {
            for (changed, normalized) in changed.iter_mut().zip(intrinsics::normalize(
                module,
                selected,
                required,
                &dispatch,
                inliner.as_mut().unwrap(),
                &mut registry,
            )) {
                *changed |= normalized;
            }
        }
        if options.interface_views {
            for (changed, reused) in changed
                .iter_mut()
                .zip(interface_views::run(module, &active))
            {
                *changed |= reused;
            }
        }
        if !changed.iter().any(|changed| *changed) {
            outcome = (round + 1, true);
            break;
        }
        active = changed;
    }
    if options.poll_chunking {
        poll_chunks::run(module, &mut registry);
    }
    module.meta.local_values = registry.finish();
    outcome
}

struct Rewrite {
    function: mir::FunctionId,
    block: mir::BlockId,
    statement: usize,
    call: mir::Call,
}

fn devirtualize(
    module: &mut mir::Module,
    selected: &mir::SelectedExternalMirSet,
    active: &[bool],
    required: Option<&intrinsics::Entries>,
    dispatch: &crate::imported_bodies::IntrinsicDispatch,
) -> Vec<bool> {
    let mut rewrites = Vec::new();
    for (function_id, function) in module.functions.iter() {
        if !active[graph::index(function_id)]
            || !function.body.blocks.iter().any(|(_, block)| {
                block.statements.iter().any(|statement| {
                    matches!(&statement.kind, mir::StatementKind::Call(effect)
                    if !matches!(call(effect).target, mir::CallTarget::Direct(_)))
                })
            })
        {
            continue;
        }
        let analysis = flow::Analysis::new(module, function);
        let incoming = analysis.solve();
        for (block_id, block) in function.body.blocks.iter() {
            let Some(mut state) = incoming[block_id.into_raw().into_u32() as usize].clone() else {
                continue;
            };
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if let mir::StatementKind::Call(effect) = &statement.kind {
                    let call = call(effect);
                    let result = match effect {
                        mir::CallEffect::Unit(_) => &mir::Type::Unit,
                        mir::CallEffect::Value { destination, .. } => {
                            &function.body.locals[*destination].ty
                        }
                    };
                    if let Some(receiver) = call.target.receiver()
                        && let Some((target, receiver_ty, unbox)) = dispatch::resolve(
                            module,
                            selected,
                            dispatch,
                            call,
                            result,
                            analysis.expression(receiver, &state).actual,
                        )
                    {
                        if required
                            .is_some_and(|required| !required.contains(module, target.into()))
                        {
                            analysis.statement(statement, &mut state);
                            continue;
                        }
                        rewrites.push(Rewrite {
                            function: function_id,
                            block: block_id,
                            statement: statement_index,
                            call: {
                                let receiver = if unbox {
                                    mir::Expr::new(
                                        receiver_ty,
                                        mir::ExprKind::Unbox(Box::new(receiver.clone())),
                                    )
                                } else if receiver.ty == receiver_ty {
                                    receiver.clone()
                                } else {
                                    mir::Expr::new(
                                        receiver_ty.clone(),
                                        mir::ExprKind::Retype {
                                            operand: Box::new(receiver.clone()),
                                            ty: Box::new(receiver_ty),
                                        },
                                    )
                                };
                                mir::Call {
                                    target: mir::CallTarget::Direct(target.into()),
                                    args: std::iter::once(receiver)
                                        .chain(call.args.iter().cloned())
                                        .collect(),
                                }
                            },
                        });
                    }
                }
                analysis.statement(statement, &mut state);
            }
        }
    }
    let mut changed = vec![false; module.functions.len()];
    for rewrite in rewrites {
        changed[graph::index(rewrite.function)] = true;
        let statement = &mut module.functions[rewrite.function].body.blocks[rewrite.block]
            .statements[rewrite.statement];
        let mir::StatementKind::Call(effect) = &mut statement.kind else {
            unreachable!("only existing calls are rewritten")
        };
        let call = match effect {
            mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
        };
        *call = rewrite.call;
    }
    changed
}

fn call(effect: &mir::CallEffect) -> &mir::Call {
    match effect {
        mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
    }
}
