//! Remove exception-only paths proven unreachable at one ordinary call site.

use std::collections::{HashMap, HashSet};

use super::{Target, flow, graph, mir, policy, simplify};
use crate::imported_bodies::{ImportedBodies, IntrinsicDispatch};
use crate::optimization::{call, dispatch};

mod operations;

pub(super) struct Effects<'a> {
    module: &'a mir::Module,
    selected: &'a mir::SelectedExternalMirSet,
    dispatch: &'a IntrinsicDispatch,
    imported: &'a ImportedBodies,
    nonthrowing: HashMap<Target, bool>,
}

impl<'a> Effects<'a> {
    pub(super) fn new(
        module: &'a mir::Module,
        selected: &'a mir::SelectedExternalMirSet,
        dispatch: &'a IntrinsicDispatch,
        imported: &'a ImportedBodies,
    ) -> Self {
        Self {
            module,
            selected,
            dispatch,
            imported,
            nonthrowing: HashMap::new(),
        }
    }

    pub(super) fn specialize(
        &mut self,
        source: &mir::Function,
        arguments: &[flow::ValueFacts],
        blocks: Vec<(mir::BlockId, mir::BasicBlock)>,
    ) -> Option<Vec<(mir::BlockId, mir::BasicBlock)>> {
        let mut function = source.clone();
        for (_, block) in function.body.blocks.iter_mut() {
            block.statements.clear();
            block.terminator = mir::Terminator::Unreachable;
            block.unwind = None;
        }
        for (id, block) in blocks {
            function.body.blocks[id] = block;
        }
        loop {
            let analysis = flow::Analysis::new(self.module, &function).with_arguments(arguments);
            let incoming = analysis.solve();
            let mut rewrites = Vec::new();
            let mut edges = Vec::new();
            for (id, block) in function.body.blocks.iter() {
                let Some(mut state) = incoming[graph::index(id)].clone() else {
                    continue;
                };
                let mut safe = block.unwind.is_some() && operations::terminator(&block.terminator);
                for (position, statement) in block.statements.iter().enumerate() {
                    safe = safe && operations::statement(statement);
                    if let mir::StatementKind::Call(effect) = &statement.kind {
                        let replacement = self.resolve(&function, effect, &analysis, &state);
                        let actual = replacement.as_ref().unwrap_or_else(|| call(effect));
                        safe = safe && self.target(actual).is_some_and(|target| self.prove(target));
                        if let Some(call) = replacement {
                            rewrites.push((id, position, call));
                        }
                    }
                    analysis.statement(statement, &mut state);
                }
                if safe {
                    edges.push(id);
                }
            }
            if edges.is_empty() {
                return None;
            }
            for (block, position, replacement) in rewrites {
                let mir::StatementKind::Call(effect) =
                    &mut function.body.blocks[block].statements[position].kind
                else {
                    unreachable!("only resolved calls are replaced")
                };
                match effect {
                    mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => {
                        *call = replacement;
                    }
                }
            }
            for block in edges {
                function.body.blocks[block].unwind = None;
            }
            let blocks = simplify::blocks(self.module, &function, arguments);
            if policy::supported_blocks(blocks.iter().map(|(_, block)| block)) {
                return Some(blocks);
            }
        }
    }

    fn target(&self, call: &mir::Call) -> Option<Target> {
        if let Some(function) = policy::local_target(self.module, call) {
            return Some(Target::Local(function));
        }
        match call.target {
            mir::CallTarget::Direct(mir::Callee::External(id))
                if self.imported.functions.contains_key(&id) =>
            {
                Some(Target::Imported(id))
            }
            _ => None,
        }
    }

    fn function(&self, target: Target) -> &'a mir::Function {
        match target {
            Target::Local(id) => &self.module.functions[id],
            Target::Imported(id) => &self.imported.functions[&id],
        }
    }

    pub(super) fn resolve(
        &self,
        function: &mir::Function,
        effect: &mir::CallEffect,
        analysis: &flow::Analysis<'_>,
        state: &flow::State,
    ) -> Option<mir::Call> {
        let call = call(effect);
        let receiver = call.target.receiver()?;
        let result = match effect {
            mir::CallEffect::Unit(_) => &mir::Type::Unit,
            mir::CallEffect::Value { destination, .. } => &function.body.locals[*destination].ty,
        };
        dispatch::direct_call(
            self.module,
            self.selected,
            self.dispatch,
            call,
            result,
            analysis.expression(receiver, state).actual,
        )
    }

    fn prove(&mut self, target: Target) -> bool {
        // A pending node is conservatively unknown. Explicit work avoids host
        // recursion for long helper chains; recursive components remain unknown.
        let mut pending: Vec<(Target, Option<Vec<Target>>)> = vec![(target, None)];
        while let Some((target, dependencies)) = pending.pop() {
            if let Some(dependencies) = dependencies {
                let proven = dependencies.iter().all(|target| self.nonthrowing[target]);
                self.nonthrowing.insert(target, proven);
                continue;
            }
            if self.nonthrowing.contains_key(&target) {
                continue;
            }
            self.nonthrowing.insert(target, false);
            if let Some(dependencies) = self.dependencies(self.function(target)) {
                let children = dependencies.clone();
                pending.push((target, Some(dependencies)));
                pending.extend(children.into_iter().map(|target| (target, None)));
            }
        }
        self.nonthrowing[&target]
    }

    fn dependencies(&self, function: &mir::Function) -> Option<Vec<Target>> {
        if !policy::executable(function) {
            return None;
        }
        let analysis = flow::Analysis::new(self.module, function);
        let incoming = analysis.solve();
        let mut pending = vec![function.body.entry];
        let mut visited = HashSet::new();
        let mut dependencies = Vec::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let mut state = incoming[graph::index(id)].clone()?;
            let block = &function.body.blocks[id];
            if !operations::terminator(&block.terminator) {
                return None;
            }
            for statement in &block.statements {
                if !operations::statement(statement)
                    || matches!(statement.kind, mir::StatementKind::Eh(_))
                {
                    return None;
                }
                if let mir::StatementKind::Call(effect) = &statement.kind {
                    let replacement = self.resolve(function, effect, &analysis, &state);
                    dependencies
                        .push(self.target(replacement.as_ref().unwrap_or_else(|| call(effect)))?);
                }
                analysis.statement(statement, &mut state);
            }
            // Once all dependencies are proven, none of this function's unwind
            // edges can execute. Only normal successors contribute to the proof.
            match &block.terminator {
                mir::Terminator::Goto(target) => pending.push(*target),
                mir::Terminator::Branch {
                    cond,
                    then_block,
                    else_block,
                } => match analysis.expression(cond, &state).constant {
                    Some(crate::optimization::constants::Constant::Boolean(value)) => {
                        pending.push(if value { *then_block } else { *else_block });
                    }
                    _ => pending.extend([*then_block, *else_block]),
                },
                _ => {}
            }
        }
        Some(dependencies)
    }
}
