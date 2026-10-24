//! Strip mine bounded scalar kernels before safepoints and roots acquire identity.

use std::collections::BTreeSet;

use scoop_identity::{StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{inline, mir};
use crate::local_values::LocalValueRegistry;

mod checks;
mod copy;
mod header;
mod linear;
mod operations;
mod recognize;
mod values;

const CHUNK: u32 = 64;

struct Loop {
    header: mir::BlockId,
    body: mir::BlockId,
    index: mir::LocalId,
    bound: mir::Expr,
    kind: mir::IntegerKind,
    header_chain: Vec<mir::BlockId>,
    body_chain: Vec<mir::BlockId>,
    temporaries: BTreeSet<mir::LocalId>,
}

pub(super) fn run(module: &mut mir::Module, registry: &mut LocalValueRegistry) {
    let functions = module
        .functions
        .iter()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    for id in functions {
        let Some(owner) = inline::owner(module, id) else {
            continue;
        };
        let function = &module.functions[id];
        if function.gc_effect != mir::GcEffect::Managed {
            continue;
        }
        let headers = function.body.loop_header_polls.clone();
        for target in headers {
            let function = &module.functions[id];
            let incoming = linear::predecessors(&function.body);
            let Some(head) = header::analyze(module, function, target.header(), &incoming) else {
                continue;
            };
            let (counted, mut projected, guards) = if let Some(counted) =
                recognize::counted(module, function, target.header(), &head, &incoming)
            {
                (counted, None, Vec::new())
            } else if let Some(projection) =
                checks::project(module, function, target.header(), &head)
            {
                (projection.counted, Some(projection.body), projection.guards)
            } else {
                continue;
            };
            let function = &mut module.functions[id];
            if guards.is_empty()
                && let Some(body) = projected.take()
            {
                function.body = body;
            }
            linear::merge(&mut function.body, &counted.header_chain);
            let source = if let Some(mut body) = projected {
                linear::merge(&mut body, &counted.body_chain);
                body.blocks[counted.body].clone()
            } else {
                linear::merge(&mut function.body, &counted.body_chain);
                function.body.blocks[counted.body].clone()
            };
            let counter = function.body.locals.alloc(mir::Local {
                name: format!("$poll.remaining.{}", function.body.locals.len()),
                ty: mir::Type::Integer(counted.kind),
                mutable: true,
            });
            registry.record_generated_local(
                id,
                counter,
                owner,
                StructuralDefinitionSiteRole::SyntheticValue,
                SyntheticLocalRole::Temporary,
            );
            let full = copy::body(function, source, &counted.temporaries, registry, owner, id);
            expand(&mut function.body, counted, counter, full, &guards);
        }
    }
}

fn expand(
    body: &mut mir::Body,
    counted: Loop,
    counter: mir::LocalId,
    full: mir::BlockId,
    guards: &[values::Length],
) {
    let Loop {
        header,
        body: inner,
        index,
        bound,
        kind,
        ..
    } = counted;
    let ty = mir::Type::Integer(kind);
    let span = body.blocks[full]
        .statements
        .last()
        .expect("counted loop updates its index")
        .span;
    let constant = |value| {
        mir::Expr::integer(if kind == mir::IntegerKind::SIGNED_32 {
            mir::MirIntegerConstant::Signed32(value)
        } else {
            mir::MirIntegerConstant::Signed64(u64::from(value))
        })
    };
    let read = || mir::Expr::local(counter, ty.clone());
    let compare = |operator, rhs| {
        mir::Expr::integer_compare(
            mir::IntegerComparisonOperation::new(kind, operator),
            read(),
            rhs,
        )
    };
    let clamp = body.blocks.alloc(mir::BasicBlock {
        name: format!("poll.clamp.{}", body.blocks.len()),
        statements: vec![mir::Statement {
            kind: mir::StatementKind::Assign {
                local: counter,
                value: constant(CHUNK),
            },
            span,
        }],
        terminator: mir::Terminator::Goto(full),
        unwind: None,
    });
    let mut full_entry = clamp;
    for length in guards.iter().rev() {
        full_entry = body.blocks.alloc(mir::BasicBlock {
            name: format!("poll.bounds.{}", body.blocks.len()),
            statements: Vec::new(),
            terminator: mir::Terminator::Branch {
                cond: mir::Expr::integer_compare(
                    mir::IntegerComparisonOperation::new(
                        kind,
                        mir::IntegerComparisonOperator::LessThanOrEqual,
                    ),
                    bound.clone(),
                    length.expression(),
                ),
                then_block: full_entry,
                else_block: inner,
            },
            unwind: None,
        });
    }
    let chunk = body.blocks.alloc(mir::BasicBlock {
        name: format!("poll.chunk.{}", body.blocks.len()),
        statements: vec![mir::Statement {
            kind: mir::StatementKind::ValDecl {
                local: counter,
                init: mir::Expr::integer_binary(
                    mir::IntegerBinaryOperation::new(kind, mir::IntegerBinaryOperator::Subtract),
                    bound,
                    mir::Expr::local(index, ty.clone()),
                ),
            },
            span,
        }],
        terminator: mir::Terminator::Branch {
            cond: compare(
                mir::IntegerComparisonOperator::GreaterThanOrEqual,
                constant(CHUNK),
            ),
            then_block: full_entry,
            else_block: inner,
        },
        unwind: None,
    });
    let mir::Terminator::Branch { then_block, .. } = &mut body.blocks[header].terminator else {
        unreachable!("recognized conditional header")
    };
    *then_block = chunk;
    for inner in std::iter::once(full).chain(guards.is_empty().then_some(inner)) {
        body.blocks[inner].statements.push(mir::Statement {
            kind: mir::StatementKind::Assign {
                local: counter,
                value: mir::Expr::integer_binary(
                    mir::IntegerBinaryOperation::new(kind, mir::IntegerBinaryOperator::Subtract),
                    read(),
                    constant(1),
                ),
            },
            span,
        });
        body.blocks[inner].terminator = mir::Terminator::Branch {
            cond: compare(mir::IntegerComparisonOperator::NotEqual, constant(0)),
            then_block: inner,
            else_block: header,
        };
    }
    // Guarded kernels retain the original checked scalar path. Otherwise the
    // scalar tail is bounded too. Only the original header remains a poll target.
}
