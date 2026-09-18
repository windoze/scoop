use scoop_ast as ast;
use scoop_hir as hir;

use super::{ExtensionPropertyCandidateOutcome, ResolvedExtensionProperty};
use crate::Lowerer;

pub(crate) enum ImplicitValueResolution {
    NoCandidate,
    NoApplicable(Box<Lowerer>),
    Failed,
    Value {
        target: crate::imports::lookup::values::ValueTarget,
        layer: crate::imports::ImportLookupLayer,
    },
    ExtensionProperty(Box<ResolvedExtensionProperty>),
}

impl Lowerer {
    /// Resolve one bare-name layer at a time after lexical and real-member
    /// lookup. Ordinary values and an extension property applicable to the
    /// implicit receiver are non-overloadable peers within the same layer.
    pub(crate) fn resolve_implicit_value(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
        require_read: bool,
    ) -> ImplicitValueResolution {
        use crate::imports::lookup::calls::NamedCallTarget;

        let mut first_failure = None;
        for layer in self.named_call_layers(&name.text) {
            let mut values = Vec::new();
            let mut properties = Vec::new();
            let mut blockers = Vec::new();
            for binding in layer.candidates {
                let origin = self.named_call_value_origin(&binding);
                match binding.target {
                    NamedCallTarget::Value(value) => values.push((value, origin)),
                    NamedCallTarget::ExtensionProperty(property)
                        if self.access_domain_allows(
                            &self.properties[property].access.lookup.0,
                            Some(receiver.ty),
                        ) =>
                    {
                        properties.push((property, origin));
                    }
                    NamedCallTarget::Function(_)
                    | NamedCallTarget::ImportedCoreCallable(_)
                    | NamedCallTarget::ImportedDependency(_)
                    | NamedCallTarget::Type(_) => {
                        blockers.push(origin);
                    }
                    NamedCallTarget::ExtensionProperty(_) => {}
                }
            }

            let extension = if properties.is_empty() {
                ExtensionPropertyCandidateOutcome::NoCandidate
            } else {
                let property_ids = properties
                    .iter()
                    .map(|(property, _)| *property)
                    .collect::<Vec<_>>();
                let mut state = self.clone();
                let mut layer_sink = Vec::new();
                match state.resolve_extension_property_candidates_outcome(
                    receiver.clone(),
                    name,
                    &property_ids,
                    &mut layer_sink,
                    require_read,
                ) {
                    ExtensionPropertyCandidateOutcome::Resolved(property) => {
                        if !values.is_empty() {
                            let mut origins = values
                                .iter()
                                .map(|(_, origin)| origin.clone())
                                .collect::<Vec<_>>();
                            origins.push(
                                properties
                                    .iter()
                                    .find_map(|(candidate, origin)| {
                                        (*candidate == property.property).then_some(origin.clone())
                                    })
                                    .expect("a resolved extension property came from this layer"),
                            );
                            self.diagnose_value_layer(name, layer.kind, &origins);
                            return ImplicitValueResolution::Failed;
                        }
                        *self = state;
                        sink.extend(layer_sink);
                        return ImplicitValueResolution::ExtensionProperty(property);
                    }
                    ExtensionPropertyCandidateOutcome::NoApplicable => {
                        first_failure.get_or_insert(Box::new(state));
                        ExtensionPropertyCandidateOutcome::NoApplicable
                    }
                    ExtensionPropertyCandidateOutcome::Failed => {
                        self.commit_layer_diagnostics(state);
                        return ImplicitValueResolution::Failed;
                    }
                    ExtensionPropertyCandidateOutcome::NoCandidate => {
                        ExtensionPropertyCandidateOutcome::NoCandidate
                    }
                }
            };

            match values.as_slice() {
                [] => {}
                [(target, _)] => {
                    return ImplicitValueResolution::Value {
                        target: *target,
                        layer: layer.kind,
                    };
                }
                _ => {
                    let origins = values
                        .iter()
                        .map(|(_, origin)| origin.clone())
                        .collect::<Vec<_>>();
                    self.diagnose_value_layer(name, layer.kind, &origins);
                    return ImplicitValueResolution::Failed;
                }
            }
            match extension {
                ExtensionPropertyCandidateOutcome::NoCandidate
                | ExtensionPropertyCandidateOutcome::NoApplicable => {}
                ExtensionPropertyCandidateOutcome::Failed
                | ExtensionPropertyCandidateOutcome::Resolved(_) => {
                    unreachable!("terminal extension outcomes return from their layer")
                }
            }
            if !blockers.is_empty() {
                self.diagnose_value_layer(name, layer.kind, &blockers);
                return ImplicitValueResolution::Failed;
            }
            if !layer.suppressed_callables.is_empty() {
                return ImplicitValueResolution::Failed;
            }
        }
        first_failure.map_or(
            ImplicitValueResolution::NoCandidate,
            ImplicitValueResolution::NoApplicable,
        )
    }
}
