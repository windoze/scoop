//! Definition-side NoTransition inference over already checked NoGc bodies.

use std::collections::{HashMap, HashSet};

use scoop_hir as hir;

use super::no_gc_generics::{GenericCall, GenericCallable};
use super::release_values::ReleaseValueFacts;
use crate::Lowerer;

mod calls;
mod expressions;
mod statements;

struct BodyFacts {
    requirements: Option<HashSet<hir::TypeParamId>>,
    calls: Vec<GenericCall>,
}

impl BodyFacts {
    fn new() -> Self {
        Self {
            requirements: Some(HashSet::new()),
            calls: Vec::new(),
        }
    }

    fn require(&mut self, condition: Option<HashSet<hir::TypeParamId>>) {
        match (&mut self.requirements, condition) {
            (Some(required), Some(next)) => required.extend(next),
            _ => self.requirements = None,
        }
    }
}

impl Lowerer {
    pub(crate) fn infer_release_callability(&mut self) {
        let values = ReleaseValueFacts::infer(self);
        let mut bodies = HashMap::new();
        for (id, function) in self.functions.iter() {
            let mut facts = BodyFacts::new();
            if function.is_suspend || function.attributes.gc_effect != hir::GcEffect::NoGc {
                facts.requirements = None;
            } else {
                facts.require(
                    values.values(
                        self,
                        function
                            .params
                            .iter()
                            .map(|parameter| parameter.ty)
                            .chain([function.return_ty]),
                    ),
                );
                match &function.kind {
                    hir::FunctionKind::User(body) => self.release_body(body, &values, &mut facts),
                    hir::FunctionKind::Intrinsic(intrinsic) if pure_intrinsic(intrinsic.kind) => {}
                    hir::FunctionKind::Intrinsic(_)
                    | hir::FunctionKind::Extern(_)
                    | hir::FunctionKind::Abstract { .. }
                    | hir::FunctionKind::InitializationEnsure
                    | hir::FunctionKind::DerivedEquality => facts.requirements = None,
                }
            }
            bodies.insert(GenericCallable::Function(id), facts);
        }
        for (id, constructor) in self.struct_constructors.iter() {
            let mut facts = BodyFacts::new();
            let hir::StructConstructorKind::Secondary {
                gc_effect: hir::GcEffect::NoGc,
                delegation,
                body,
            } = &constructor.kind
            else {
                facts.requirements = None;
                bodies.insert(GenericCallable::StructConstructor(id), facts);
                continue;
            };
            let owner = &self.structs[constructor.owner];
            let result = self.struct_applications[owner.self_application].canonical_type;
            facts.require(
                values.values(
                    self,
                    constructor
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .chain([result]),
                ),
            );
            facts.require(values.values(
                self,
                delegation.arguments.locals.values().map(|local| local.ty),
            ));
            self.release_statements(&delegation.arguments.statements, &values, &mut facts);
            for argument in &delegation.arguments.args {
                self.release_expression(argument, &values, &mut facts);
            }
            self.release_constructor_call(delegation.target, constructor.span, &values, &mut facts);
            self.release_body(body, &values, &mut facts);
            bodies.insert(GenericCallable::StructConstructor(id), facts);
        }
        let mut nodes = bodies.keys().copied().collect::<Vec<_>>();
        nodes.sort_unstable();
        let graph = bodies
            .iter()
            .map(|(&node, body)| (node, body.calls.iter().map(|call| call.callee).collect()))
            .collect();
        let mut effects = bodies
            .iter()
            .map(|(&node, body)| (node, body.requirements.clone()))
            .collect::<HashMap<_, _>>();
        for component in crate::graph::strongly_connected_components(&nodes, &graph)
            .into_iter()
            .rev()
        {
            loop {
                let mut changed = false;
                for &node in &component {
                    let mut next = BodyFacts::new();
                    next.require(bodies[&node].requirements.clone());
                    for call in &bodies[&node].calls {
                        let mapped = effects[&call.callee].as_ref().and_then(|parameters| {
                            values.values(
                                self,
                                parameters.iter().map(|parameter| {
                                    call.arguments
                                        .iter()
                                        .find_map(|(key, ty)| (key == parameter).then_some(*ty))
                                        .expect("a resolved call binds each required parameter")
                                }),
                            )
                        });
                        next.require(mapped);
                    }
                    if effects[&node] != next.requirements {
                        effects.insert(node, next.requirements);
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
        }
        for (node, required) in effects {
            let effect = match required {
                Some(required) => {
                    let mut requirements = required.into_iter().collect::<Vec<_>>();
                    requirements.sort_by_key(|parameter| parameter.into_raw());
                    hir::ReleaseCallability::NoTransition { requirements }
                }
                None => hir::ReleaseCallability::Unavailable,
            };
            match node {
                GenericCallable::Function(id) => self.functions[id].release_callability = effect,
                GenericCallable::StructConstructor(id) => {
                    self.struct_constructors[id].release_callability = effect;
                }
                GenericCallable::Imported(_)
                | GenericCallable::ImportedConstructor(_)
                | GenericCallable::ClassConstructor(_) => {
                    unreachable!("only current value callables enter release inference")
                }
            }
        }
    }
}

fn pure_intrinsic(kind: hir::IntrinsicFunctionKind) -> bool {
    matches!(
        kind,
        hir::IntrinsicFunctionKind::Integer(
            hir::IntegerIntrinsicKind::NoGcOperation { .. }
                | hir::IntegerIntrinsicKind::Conversion { .. }
        ) | hir::IntrinsicFunctionKind::Pointer(_)
            | hir::IntrinsicFunctionKind::PrimitiveUnary(hir::PrimitiveUnaryKind::BooleanNot)
    )
}
