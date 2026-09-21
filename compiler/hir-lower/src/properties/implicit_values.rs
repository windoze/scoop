use scoop_ast as ast;
use scoop_hir as hir;

use super::{
    ExtensionPropertyCandidateOutcome, ResolvedExtensionPropertyRead,
    ResolvedExtensionPropertyWrite,
};
use crate::Lowerer;
use crate::imports::lookup::calls::{ExtensionPropertyTarget, NamedCallOrigin, NamedCallTarget};

pub(crate) enum ImplicitValueResolution<T> {
    NoCandidate,
    NoApplicable(Box<Lowerer>),
    Failed,
    Value {
        target: crate::imports::lookup::values::ValueTarget,
        layer: crate::imports::ImportLookupLayer,
    },
    ExtensionProperty(Box<T>),
}

#[derive(Clone, Copy)]
enum ImplicitValueAccess {
    Read,
    Write,
}

enum SelectedImplicitExtensionProperty {
    Read(ResolvedExtensionPropertyRead),
    Write(ResolvedExtensionPropertyWrite),
}

enum ImplicitValueSelection {
    NoCandidate,
    NoApplicable(Box<Lowerer>),
    Failed,
    Value {
        target: crate::imports::lookup::values::ValueTarget,
        layer: crate::imports::ImportLookupLayer,
    },
    ExtensionProperty(Box<SelectedImplicitExtensionProperty>),
}

impl Lowerer {
    pub(crate) fn resolve_implicit_value_read(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> ImplicitValueResolution<ResolvedExtensionPropertyRead> {
        match self.resolve_implicit_value_for(receiver, name, sink, ImplicitValueAccess::Read) {
            ImplicitValueSelection::NoCandidate => ImplicitValueResolution::NoCandidate,
            ImplicitValueSelection::NoApplicable(failure) => {
                ImplicitValueResolution::NoApplicable(failure)
            }
            ImplicitValueSelection::Failed => ImplicitValueResolution::Failed,
            ImplicitValueSelection::Value { target, layer } => {
                ImplicitValueResolution::Value { target, layer }
            }
            ImplicitValueSelection::ExtensionProperty(property) => match *property {
                SelectedImplicitExtensionProperty::Read(property) => {
                    ImplicitValueResolution::ExtensionProperty(Box::new(property))
                }
                SelectedImplicitExtensionProperty::Write(_) => {
                    unreachable!("implicit read lookup produces a readable property")
                }
            },
        }
    }

    pub(crate) fn resolve_implicit_value_write(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> ImplicitValueResolution<ResolvedExtensionPropertyWrite> {
        match self.resolve_implicit_value_for(receiver, name, sink, ImplicitValueAccess::Write) {
            ImplicitValueSelection::NoCandidate => ImplicitValueResolution::NoCandidate,
            ImplicitValueSelection::NoApplicable(failure) => {
                ImplicitValueResolution::NoApplicable(failure)
            }
            ImplicitValueSelection::Failed => ImplicitValueResolution::Failed,
            ImplicitValueSelection::Value { target, layer } => {
                ImplicitValueResolution::Value { target, layer }
            }
            ImplicitValueSelection::ExtensionProperty(property) => match *property {
                SelectedImplicitExtensionProperty::Write(property) => {
                    ImplicitValueResolution::ExtensionProperty(Box::new(property))
                }
                SelectedImplicitExtensionProperty::Read(_) => {
                    unreachable!("implicit write lookup produces a property write target")
                }
            },
        }
    }

    /// Resolve one bare-name layer at a time after lexical and real-member
    /// lookup. Ordinary values and an extension property applicable to the
    /// implicit receiver are non-overloadable peers within the same layer.
    fn resolve_implicit_value_for(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
        access: ImplicitValueAccess,
    ) -> ImplicitValueSelection {
        let mut first_failure = None;
        for layer in self.named_call_layers(&name.text) {
            let mut values = Vec::new();
            let mut properties = Vec::new();
            let mut blockers = Vec::new();
            for binding in layer.candidates {
                let origin = self.named_call_value_origin(&binding);
                match (binding.target, binding.origin) {
                    (NamedCallTarget::Value(value), _) => values.push((value, origin)),
                    (NamedCallTarget::ExtensionProperty(property), _) => {
                        properties.push((ExtensionPropertyTarget::Current(property), origin))
                    }
                    (
                        NamedCallTarget::ImportedDependency(
                            hir::ImportedTarget::ExtensionProperty(_),
                        ),
                        NamedCallOrigin::Dependency(binding),
                    ) => properties.push((ExtensionPropertyTarget::Dependency(binding), origin)),
                    (
                        NamedCallTarget::Function(_)
                        | NamedCallTarget::ImportedDependency(_)
                        | NamedCallTarget::Type(_),
                        _,
                    ) => blockers.push(origin),
                }
            }

            let extension = if properties.is_empty() {
                ExtensionPropertySelection::NoCandidate
            } else {
                let targets = properties
                    .iter()
                    .map(|(property, _)| property.clone())
                    .collect::<Vec<_>>();
                let mut state = self.clone();
                let mut layer_sink = Vec::new();
                let outcome = match access {
                    ImplicitValueAccess::Read => state
                        .resolve_extension_property_candidates_outcome(
                            receiver.clone(),
                            name,
                            &targets,
                            &mut layer_sink,
                        )
                        .map_read(),
                    ImplicitValueAccess::Write => state
                        .resolve_extension_property_write_candidates_outcome(
                            receiver.clone(),
                            name,
                            &targets,
                            &mut layer_sink,
                        )
                        .map_write(),
                };
                match outcome {
                    ExtensionPropertySelection::Resolved(property) => {
                        if !values.is_empty() {
                            let mut origins = values
                                .iter()
                                .map(|(_, origin)| origin.clone())
                                .collect::<Vec<_>>();
                            origins.push(
                                properties
                                    .iter()
                                    .find_map(|(candidate, origin)| {
                                        (candidate.identity() == property.identity())
                                            .then_some(origin.clone())
                                    })
                                    .expect("a resolved extension property came from this layer"),
                            );
                            self.diagnose_value_layer(name, layer.kind, &origins);
                            return ImplicitValueSelection::Failed;
                        }
                        *self = state;
                        sink.extend(layer_sink);
                        return ImplicitValueSelection::ExtensionProperty(property);
                    }
                    ExtensionPropertySelection::NoApplicable => {
                        first_failure.get_or_insert(Box::new(state));
                        ExtensionPropertySelection::NoApplicable
                    }
                    ExtensionPropertySelection::Failed => {
                        self.commit_layer_diagnostics(state);
                        return ImplicitValueSelection::Failed;
                    }
                    ExtensionPropertySelection::NoCandidate => {
                        ExtensionPropertySelection::NoCandidate
                    }
                }
            };

            match values.as_slice() {
                [] => {}
                [(target, _)] => {
                    return ImplicitValueSelection::Value {
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
                    return ImplicitValueSelection::Failed;
                }
            }
            match extension {
                ExtensionPropertySelection::NoCandidate
                | ExtensionPropertySelection::NoApplicable => {}
                ExtensionPropertySelection::Failed | ExtensionPropertySelection::Resolved(_) => {
                    unreachable!("terminal extension outcomes return from their layer")
                }
            }
            if !blockers.is_empty() {
                self.diagnose_value_layer(name, layer.kind, &blockers);
                return ImplicitValueSelection::Failed;
            }
            if !layer.suppressed_callables.is_empty() {
                return ImplicitValueSelection::Failed;
            }
        }
        first_failure.map_or(
            ImplicitValueSelection::NoCandidate,
            ImplicitValueSelection::NoApplicable,
        )
    }
}

enum ExtensionPropertySelection {
    NoCandidate,
    NoApplicable,
    Failed,
    Resolved(Box<SelectedImplicitExtensionProperty>),
}

impl<T> ExtensionPropertyCandidateOutcome<T> {
    fn map_read(self) -> ExtensionPropertySelection
    where
        T: Into<ResolvedExtensionPropertyRead>,
    {
        match self {
            Self::NoCandidate => ExtensionPropertySelection::NoCandidate,
            Self::NoApplicable => ExtensionPropertySelection::NoApplicable,
            Self::Failed => ExtensionPropertySelection::Failed,
            Self::Resolved(property) => ExtensionPropertySelection::Resolved(Box::new(
                SelectedImplicitExtensionProperty::Read((*property).into()),
            )),
        }
    }

    fn map_write(self) -> ExtensionPropertySelection
    where
        T: Into<ResolvedExtensionPropertyWrite>,
    {
        match self {
            Self::NoCandidate => ExtensionPropertySelection::NoCandidate,
            Self::NoApplicable => ExtensionPropertySelection::NoApplicable,
            Self::Failed => ExtensionPropertySelection::Failed,
            Self::Resolved(property) => ExtensionPropertySelection::Resolved(Box::new(
                SelectedImplicitExtensionProperty::Write((*property).into()),
            )),
        }
    }
}

impl SelectedImplicitExtensionProperty {
    const fn identity(&self) -> crate::imports::lookup::calls::ExtensionPropertyIdentity {
        match self {
            Self::Read(property) => property.identity(),
            Self::Write(property) => property.identity(),
        }
    }
}
