//! Structural termination check for generic callable specialization.

use std::collections::{HashMap, HashSet, VecDeque};

use scoop_hir as hir;

use super::no_gc_generics::{GenericCallSite, GenericCallable};
use crate::Lowerer;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SymbolicType {
    Unit,
    Integer(hir::IntegerKind),
    Boolean,
    String,
    Struct(hir::SourceNominalId, Vec<SymbolicType>),
    ImportedNominal(hir::SourceNominalId, Vec<SymbolicType>),
    Class(hir::SourceNominalId, Vec<SymbolicType>),
    Interface(hir::InterfaceId, Vec<SymbolicType>),
    Any,
    Tuple(Vec<SymbolicType>),
    Function {
        is_suspend: bool,
        parameters: Vec<SymbolicType>,
        result: Box<SymbolicType>,
    },
    Ptr(Box<SymbolicType>),
    FunPtr {
        parameters: Vec<SymbolicType>,
        result: Box<SymbolicType>,
    },
    Enum(hir::SourceNominalId, Vec<SymbolicType>),
    Parameter(hir::TypeParamId),
}

impl Lowerer {
    pub(crate) fn check_generic_recursion(&mut self) {
        let mut node_order = self
            .effect_callable_ids()
            .into_iter()
            .filter(|id| !self.effect_callable_parameters(*id).is_empty())
            .collect::<Vec<_>>();
        node_order.sort();
        let nodes = node_order.iter().copied().collect::<HashSet<_>>();
        let call_sites = self
            .generic_call_sites()
            .into_iter()
            .filter(|call| nodes.contains(&call.caller) && nodes.contains(&call.callee))
            .collect::<Vec<_>>();
        if call_sites.is_empty() {
            return;
        }

        let mut forward = HashMap::<GenericCallable, Vec<GenericCallable>>::new();
        let mut reverse = HashMap::<GenericCallable, Vec<GenericCallable>>::new();
        for call in &call_sites {
            forward.entry(call.caller).or_default().push(call.callee);
            reverse.entry(call.callee).or_default().push(call.caller);
        }

        let mut seen = HashSet::new();
        let mut order = Vec::new();
        for node in node_order {
            finish_order(node, &forward, &mut seen, &mut order);
        }
        seen.clear();
        let mut components = Vec::new();
        while let Some(node) = order.pop() {
            if seen.contains(&node) {
                continue;
            }
            let mut component = Vec::new();
            collect_component(node, &reverse, &mut seen, &mut component);
            components.push(component);
        }

        let mut violations = Vec::new();
        for component in components {
            let members = component.iter().copied().collect::<HashSet<_>>();
            let recursive = component.len() > 1
                || call_sites
                    .iter()
                    .any(|call| call.caller == component[0] && call.callee == component[0]);
            if !recursive {
                continue;
            }

            let root = component[0];
            let root_mapping = self
                .effect_callable_parameters(root)
                .into_iter()
                .map(|parameter| SymbolicType::Parameter(parameter.id))
                .collect::<Vec<_>>();
            let mut mappings = HashMap::from([(root, root_mapping)]);
            let mut queue = VecDeque::from([root]);
            let mut reported = HashSet::new();
            while let Some(caller) = queue.pop_front() {
                let caller_mapping = mappings[&caller].clone();
                let caller_bindings = self
                    .effect_callable_parameters(caller)
                    .into_iter()
                    .map(|parameter| parameter.id)
                    .zip(caller_mapping)
                    .collect::<HashMap<_, _>>();
                for (edge_index, call) in call_sites.iter().enumerate() {
                    if call.caller != caller || !members.contains(&call.callee) {
                        continue;
                    }
                    let callee_mapping = self.call_mapping(call, &caller_bindings);
                    match mappings.get(&call.callee) {
                        None => {
                            mappings.insert(call.callee, callee_mapping);
                            queue.push_back(call.callee);
                        }
                        Some(existing) if existing != &callee_mapping => {
                            if reported.insert(edge_index) {
                                violations.push((
                                    call.caller,
                                    call.span,
                                    component
                                        .iter()
                                        .map(|function| self.effect_callable_name(*function))
                                        .collect::<Vec<_>>()
                                        .join(" -> "),
                                ));
                            }
                        }
                        Some(_) => {}
                    }
                }
            }
        }

        for (caller, span, cycle) in violations {
            self.current_file = self.effect_callable_file(caller);
            self.error(
                span,
                format!(
                    "polymorphic recursion is not supported: generic callable cycle `{cycle}` changes its complete type-argument mapping"
                ),
            );
        }
    }

    fn call_mapping(
        &self,
        call: &GenericCallSite,
        caller_bindings: &HashMap<hir::TypeParamId, SymbolicType>,
    ) -> Vec<SymbolicType> {
        self.effect_callable_parameters(call.callee)
            .into_iter()
            .map(|parameter| {
                let argument = call
                    .arguments
                    .iter()
                    .find_map(|(candidate, argument)| {
                        (*candidate == parameter.id).then_some(*argument)
                    })
                    .expect("an exact callable application binds every callee parameter");
                self.symbolic_type(argument, caller_bindings)
            })
            .collect()
    }

    fn symbolic_type(
        &self,
        ty: hir::TypeId,
        bindings: &HashMap<hir::TypeParamId, SymbolicType>,
    ) -> SymbolicType {
        match &self.types[ty] {
            hir::Type::ImportedInterface(structure) => SymbolicType::ImportedNominal(
                structure.declaration.owner(),
                structure
                    .arguments
                    .iter()
                    .map(|argument| self.symbolic_type(*argument, bindings))
                    .collect(),
            ),
            hir::Type::Unit => SymbolicType::Unit,
            hir::Type::Integer(kind) => SymbolicType::Integer(*kind),
            hir::Type::Boolean => SymbolicType::Boolean,
            hir::Type::String => SymbolicType::String,
            hir::Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                SymbolicType::Struct(
                    application.template,
                    application
                        .arguments
                        .iter()
                        .map(|argument| self.symbolic_type(*argument, bindings))
                        .collect(),
                )
            }
            hir::Type::Class(application) => {
                let application = &self.class_applications[*application];
                SymbolicType::Class(
                    application.template,
                    application
                        .arguments
                        .iter()
                        .map(|argument| self.symbolic_type(*argument, bindings))
                        .collect(),
                )
            }
            hir::Type::Interface(application) => {
                let application = &self.interface_applications[*application];
                SymbolicType::Interface(
                    self.interface_id(application.template),
                    application
                        .arguments
                        .iter()
                        .map(|argument| self.symbolic_type(*argument, bindings))
                        .collect(),
                )
            }
            hir::Type::Any => SymbolicType::Any,
            hir::Type::Tuple(elements) => SymbolicType::Tuple(
                elements
                    .iter()
                    .map(|element| self.symbolic_type(*element, bindings))
                    .collect(),
            ),
            hir::Type::Function(function) => {
                let function = &self.function_types[*function];
                SymbolicType::Function {
                    is_suspend: function.is_suspend,
                    parameters: function
                        .parameter_types
                        .iter()
                        .map(|parameter| self.symbolic_type(*parameter, bindings))
                        .collect(),
                    result: Box::new(self.symbolic_type(function.return_type, bindings)),
                }
            }
            hir::Type::Ptr(pointee) => {
                SymbolicType::Ptr(Box::new(self.symbolic_type(*pointee, bindings)))
            }
            hir::Type::FunPtr(function) => {
                let function = &self.function_types[*function];
                SymbolicType::FunPtr {
                    parameters: function
                        .parameter_types
                        .iter()
                        .map(|parameter| self.symbolic_type(*parameter, bindings))
                        .collect(),
                    result: Box::new(self.symbolic_type(function.return_type, bindings)),
                }
            }
            hir::Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                SymbolicType::Enum(
                    application.template,
                    application
                        .arguments
                        .iter()
                        .map(|argument| self.symbolic_type(*argument, bindings))
                        .collect(),
                )
            }
            hir::Type::Param(parameter) => bindings
                .get(parameter)
                .cloned()
                .expect("the caller mapping binds every referenced type parameter"),
        }
    }
}

fn finish_order(
    node: GenericCallable,
    graph: &HashMap<GenericCallable, Vec<GenericCallable>>,
    seen: &mut HashSet<GenericCallable>,
    order: &mut Vec<GenericCallable>,
) {
    if !seen.insert(node) {
        return;
    }
    for &next in graph.get(&node).into_iter().flatten() {
        finish_order(next, graph, seen, order);
    }
    order.push(node);
}

fn collect_component(
    node: GenericCallable,
    graph: &HashMap<GenericCallable, Vec<GenericCallable>>,
    seen: &mut HashSet<GenericCallable>,
    component: &mut Vec<GenericCallable>,
) {
    if !seen.insert(node) {
        return;
    }
    component.push(node);
    for &next in graph.get(&node).into_iter().flatten() {
        collect_component(next, graph, seen, component);
    }
}
