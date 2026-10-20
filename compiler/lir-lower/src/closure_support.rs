//! Select physical support for ordinary private closure environments.

use std::collections::{HashMap, HashSet, VecDeque};

use scoop_identity::{CallableTemplateOwner, GeneratedCallableKey};
use scoop_mir as mir;

mod references;

pub(crate) struct ClosureSupport {
    functions: HashSet<mir::FunctionId>,
    closures: HashSet<mir::ClosureClassId>,
}

impl ClosureSupport {
    pub(crate) fn new(module: &mir::Module) -> Self {
        let mut owners: HashMap<_, Vec<_>> = HashMap::new();
        let mut candidates = HashSet::new();
        for (class, definition) in module.closure_classes.iter() {
            let functions = support_functions(module, definition);
            for function in functions {
                owners.entry(function).or_default().push(class);
                if !module.function_types[definition.function_type].is_suspend
                    && private_helper(module, function)
                {
                    candidates.insert(function);
                }
            }
        }
        let mut selected = Self {
            functions: HashSet::new(),
            closures: HashSet::new(),
        };
        let mut pending = VecDeque::new();
        for (class, definition) in module.closure_classes.iter() {
            if module.function_types[definition.function_type].is_suspend {
                selected.closure(module, class, &mut pending);
            }
        }
        for &function in &module.top_level {
            if !candidates.contains(&function) {
                selected.function(function, &mut pending);
            }
        }
        references::roots(module, &mut |function| {
            selected.function(function, &mut pending);
        });
        for (_, hook) in module.release_hooks.iter() {
            selected.body(module, &hook.code.body, &mut pending);
        }
        while let Some(function) = pending.pop_front() {
            if let Some(classes) = owners.get(&function) {
                for &class in classes {
                    selected.closure(module, class, &mut pending);
                }
            }
            selected.body(module, &module.functions[function].body, &mut pending);
        }
        selected
    }

    pub(crate) fn emits_function(&self, function: mir::FunctionId) -> bool {
        self.functions.contains(&function)
    }

    pub(crate) fn emits_closure(&self, class: mir::ClosureClassId) -> bool {
        self.closures.contains(&class)
    }

    fn function(&mut self, function: mir::FunctionId, pending: &mut VecDeque<mir::FunctionId>) {
        if self.functions.insert(function) {
            pending.push_back(function);
        }
    }

    fn closure(
        &mut self,
        module: &mir::Module,
        class: mir::ClosureClassId,
        pending: &mut VecDeque<mir::FunctionId>,
    ) {
        if self.closures.insert(class) {
            for function in support_functions(module, &module.closure_classes[class]) {
                self.function(function, pending);
            }
        }
    }

    fn body(
        &mut self,
        module: &mir::Module,
        body: &mir::Body,
        pending: &mut VecDeque<mir::FunctionId>,
    ) {
        for (_, block) in body.blocks.iter() {
            for statement in &block.statements {
                if let mir::StatementKind::Call(
                    mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. },
                ) = &statement.kind
                {
                    let callee = match call.target {
                        mir::CallTarget::Direct(callee) => Some(callee),
                        mir::CallTarget::Virtual { callee, .. }
                        | mir::CallTarget::Interface { callee, .. } => Some(callee.into()),
                        _ => None,
                    };
                    match callee {
                        Some(mir::Callee::User(function)) => self.function(function, pending),
                        Some(mir::Callee::Monomorphized(instance)) => {
                            self.function(module.meta.instances[instance].function, pending);
                        }
                        _ => {}
                    }
                }
            }
            mir::visit_block_exprs(block, &mut |expression| match expression.kind {
                mir::ExprKind::ClosureAlloc { class, .. }
                | mir::ExprKind::ClosureCapture { class, .. } => {
                    self.closure(module, class, pending);
                }
                _ => {}
            });
        }
    }
}

fn support_functions<'a>(
    module: &'a mir::Module,
    class: &'a mir::ClosureClass,
) -> impl Iterator<Item = mir::FunctionId> + 'a {
    [module.closure_invoke_functions[class.invoke].function]
        .into_iter()
        .chain(class.bridges.iter().map(|bridge| bridge.function))
}

fn private_helper(module: &mir::Module, function: mir::FunctionId) -> bool {
    // Source lexical closures retain their HIR-generated template identity.
    // An ordinary source callable is never made private by this association.
    if let Some(source) = module.meta.source_callable_materializations.get(function) {
        return matches!(
            source.materialization().template(),
            CallableTemplateOwner::Generated(_)
        );
    }
    module
        .meta
        .generated_callables
        .get(function)
        .is_some_and(|record| {
            matches!(
                record.identity_record().key(),
                GeneratedCallableKey::CallableReferenceInvoke { .. }
                    | GeneratedCallableKey::FunctionBridge { .. }
                    | GeneratedCallableKey::FunctionAdapter { .. }
                    | GeneratedCallableKey::DynamicFunctionAdapter { .. }
            )
        })
}
