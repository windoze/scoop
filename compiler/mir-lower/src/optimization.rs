//! Local optimizations over complete MIR, before output validation and LIR.

use scoop_mir as mir;

mod constants;
mod dispatch;
mod flow;
mod graph;
mod inline;
mod simplify;

/// Producer choices; they do not change entity or ABI identity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MirOptimizationOptions {
    pub devirtualize: bool,
    pub inline: bool,
}

impl MirOptimizationOptions {
    pub const RELEASE: Self = Self {
        devirtualize: true,
        inline: true,
    };
}

pub(super) fn run(
    module: &mut mir::Module,
    selected: &mir::SelectedExternalMirSet,
    options: MirOptimizationOptions,
) {
    if options.devirtualize {
        devirtualize(module, selected);
    }
    if options.inline {
        inline::run(module);
    }
    if options.devirtualize && options.inline {
        devirtualize(module, selected);
    }
}

struct Rewrite {
    function: mir::FunctionId,
    block: mir::BlockId,
    statement: usize,
    target: mir::Callee,
    receiver: mir::Type,
}

fn devirtualize(module: &mut mir::Module, selected: &mir::SelectedExternalMirSet) {
    let mut rewrites = Vec::new();
    for (function_id, function) in module.functions.iter() {
        if !function.body.blocks.iter().any(|(_, block)| {
            block.statements.iter().any(|statement| {
                matches!(&statement.kind, mir::StatementKind::Call(effect)
                    if !matches!(call(effect).target.kind, mir::CallKind::Direct))
            })
        }) {
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
                    if let Some(receiver) = call.args.first()
                        && let Some((target, receiver)) = dispatch::resolve(
                            module,
                            selected,
                            call,
                            result,
                            analysis.expression(receiver, &state).actual,
                        )
                    {
                        rewrites.push(Rewrite {
                            function: function_id,
                            block: block_id,
                            statement: statement_index,
                            target,
                            receiver,
                        });
                    }
                }
                analysis.statement(statement, &mut state);
            }
        }
    }
    for rewrite in rewrites {
        let statement = &mut module.functions[rewrite.function].body.blocks[rewrite.block]
            .statements[rewrite.statement];
        let mir::StatementKind::Call(effect) = &mut statement.kind else {
            unreachable!("only existing calls are rewritten")
        };
        let call = match effect {
            mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
        };
        if call.args[0].ty != rewrite.receiver {
            let operand = call.args[0].clone();
            call.args[0] = mir::Expr::new(
                rewrite.receiver.clone(),
                mir::ExprKind::Retype {
                    operand: Box::new(operand),
                    ty: Box::new(rewrite.receiver),
                },
            );
        }
        call.target = mir::CallTarget {
            kind: mir::CallKind::Direct,
            callee: rewrite.target,
        };
    }
}

fn call(effect: &mir::CallEffect) -> &mir::Call {
    match effect {
        mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
    }
}
