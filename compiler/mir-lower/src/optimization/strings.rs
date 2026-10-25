//! Fuse single-use concatenation trees without crossing observable operations.

use std::collections::{HashMap, HashSet};

use super::{allocation, graph, inline, mir};

const MAX_PARTS: usize = 8;
const MAX_LITERAL_BYTES: usize = 64;

#[derive(Clone)]
struct Origin {
    parts: Vec<mir::StringConcatPart>,
    removed: Vec<usize>,
    concatenations: usize,
}

#[derive(Default)]
struct Uses {
    reads: HashMap<mir::LocalId, usize>,
    writes: HashMap<mir::LocalId, usize>,
    addressed: HashSet<mir::LocalId>,
}

impl Uses {
    fn new(function: &mir::Function) -> Self {
        let mut uses = Self::default();
        for parameter in &function.params {
            *uses.writes.entry(parameter.local).or_default() += 1;
        }
        for (_, block) in function.body.blocks.iter() {
            for statement in &block.statements {
                if let Some(local) = allocation::defined_local(statement) {
                    *uses.writes.entry(local).or_default() += 1;
                }
            }
            mir::visit_block_exprs(block, &mut |expr| match expr.kind {
                mir::ExprKind::Local(local) => *uses.reads.entry(local).or_default() += 1,
                mir::ExprKind::AddressOf { local, .. } => {
                    uses.addressed.insert(local);
                }
                _ => {}
            });
        }
        uses
    }

    fn stable(&self, local: mir::LocalId) -> bool {
        self.writes.get(&local) == Some(&1) && !self.addressed.contains(&local)
    }
}

pub(super) fn run(module: &mut mir::Module) {
    for (_, function) in module.functions.iter_mut() {
        if !function.body.blocks.iter().any(|(_, block)| block.statements.iter().any(|statement| {
            matches!(&statement.kind, mir::StatementKind::Call(mir::CallEffect::Value { call, .. })
                if matches!(call.target, mir::CallTarget::Direct(mir::Callee::Runtime(mir::RuntimeFn::StringConcat))))
        })) {
            continue;
        }
        let uses = Uses::new(function);
        let heat = inline::heats(&function.body);
        for (id, block) in function.body.blocks.iter_mut() {
            let site = heat[graph::index(id)];
            fuse(
                block,
                &uses,
                &module.strings,
                site.numerator > site.denominator,
            );
        }
    }
}

fn fuse(
    block: &mut mir::BasicBlock,
    uses: &Uses,
    strings: &la_arena::Arena<mir::StringConst>,
    hot: bool,
) {
    let mut origins: HashMap<mir::LocalId, Origin> = HashMap::new();
    let mut removed = HashSet::new();
    for index in 0..block.statements.len() {
        let statement = &block.statements[index];
        if let mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) =
            &statement.kind
            && matches!(
                call.target,
                mir::CallTarget::Direct(mir::Callee::Runtime(mir::RuntimeFn::StringConcat))
            )
            && let [left, right] = call.args.as_slice()
            && uses.stable(*destination)
        {
            let left = origin(left, uses, strings, &origins);
            let right = origin(right, uses, strings, &origins);
            if let (Some(mut chain), Some(right)) = (left, right) {
                chain.concatenations += right.concatenations + 1;
                chain.removed.extend(right.removed);
                for part in right.parts {
                    append(&mut chain.parts, part);
                }
                if chain.parts.len() <= MAX_PARTS {
                    let destination = *destination;
                    if chain.concatenations > 1
                        || hot
                        || chain
                            .parts
                            .iter()
                            .all(|part| matches!(part, mir::StringConcatPart::Literal(_)))
                    {
                        removed.extend(chain.removed.iter().copied());
                        block.statements[index].kind = mir::StatementKind::ValDecl {
                            local: destination,
                            init: mir::Expr {
                                ty: mir::Type::String,
                                kind: mir::ExprKind::StringConcat(chain.parts.clone()),
                            },
                        };
                    }
                    chain.removed.push(index);
                    origins.insert(destination, chain);
                    continue;
                }
            }
        }
        if let mir::StatementKind::ValDecl { local, init }
        | mir::StatementKind::Assign { local, value: init } = &statement.kind
            && uses.stable(*local)
            && let Some(mut alias) = origin(init, uses, strings, &origins)
        {
            alias.removed.push(index);
            origins.insert(*local, alias);
            continue;
        }
        // Even a NoGC call can throw, mutate a binding, or expose an allocation.
        origins.clear();
    }
    let mut index = 0;
    block.statements.retain(|_| {
        let keep = !removed.contains(&index);
        index += 1;
        keep
    });
}

fn origin(
    expr: &mir::Expr,
    uses: &Uses,
    strings: &la_arena::Arena<mir::StringConst>,
    origins: &HashMap<mir::LocalId, Origin>,
) -> Option<Origin> {
    let part = match expr.kind {
        mir::ExprKind::Local(local) if uses.stable(local) && expr.ty == mir::Type::String => {
            if uses.reads.get(&local) == Some(&1)
                && let Some(origin) = origins.get(&local)
            {
                return Some(origin.clone());
            }
            mir::StringConcatPart::Value(expr.clone())
        }
        mir::ExprKind::StringConst(id) => {
            let text = &strings[id].value;
            if text.len() <= MAX_LITERAL_BYTES {
                mir::StringConcatPart::Literal(text.clone())
            } else {
                mir::StringConcatPart::Value(expr.clone())
            }
        }
        _ => return None,
    };
    Some(Origin {
        parts: vec![part],
        removed: Vec::new(),
        concatenations: 0,
    })
}

fn append(parts: &mut Vec<mir::StringConcatPart>, part: mir::StringConcatPart) {
    if let mir::StringConcatPart::Literal(right) = &part
        && let Some(mir::StringConcatPart::Literal(left)) = parts.last_mut()
        && left.len() + right.len() <= MAX_LITERAL_BYTES
    {
        left.push_str(right);
    } else {
        parts.push(part);
    }
}
