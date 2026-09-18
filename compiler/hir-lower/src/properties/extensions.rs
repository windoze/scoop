use scoop_ast as ast;
use scoop_hir as hir;

use crate::imports::lookup::calls::{ExtensionPropertyIdentity, ExtensionPropertyTarget};
use crate::{Lowerer, TypeId};

mod candidates;
mod write;

#[derive(Clone)]
pub(crate) struct ResolvedExtensionPropertyRead {
    pub(crate) read: hir::Expr,
    pub(crate) write: ResolvedExtensionPropertyWrite,
}

impl ResolvedExtensionPropertyRead {
    pub(crate) const fn identity(&self) -> ExtensionPropertyIdentity {
        self.write.identity()
    }
}

#[derive(Clone)]
pub(crate) struct ResolvedExtensionPropertyWrite {
    target: ResolvedExtensionPropertyTarget,
    receiver: hir::Expr,
    value_type: TypeId,
    has_setter: bool,
}

impl ResolvedExtensionPropertyWrite {
    pub(crate) const fn identity(&self) -> ExtensionPropertyIdentity {
        match &self.target {
            ResolvedExtensionPropertyTarget::Current { property, .. } => {
                ExtensionPropertyIdentity::Current(*property)
            }
            ResolvedExtensionPropertyTarget::Dependency { binding, .. } => {
                ExtensionPropertyIdentity::Dependency(binding.target())
            }
        }
    }

    pub(crate) const fn value_type(&self) -> TypeId {
        self.value_type
    }

    pub(crate) const fn has_setter(&self) -> bool {
        self.has_setter
    }
}

#[derive(Clone)]
enum ResolvedExtensionPropertyTarget {
    Current {
        property: hir::PropertyId,
        type_args: Vec<TypeId>,
    },
    Dependency {
        binding: hir::DirectImportedTargetBinding,
        name: ast::Ident,
    },
}

pub(crate) enum ExtensionPropertyResolution<T> {
    NoCandidate,
    Failed,
    Resolved(Box<T>),
}

pub(crate) enum ExtensionPropertyCandidateOutcome<T> {
    NoCandidate,
    NoApplicable,
    Failed,
    Resolved(Box<T>),
}

#[derive(Clone, Copy)]
pub(super) enum ExtensionPropertyAccess {
    Read,
    Write,
}

pub(super) enum SelectedExtensionProperty {
    Read(ResolvedExtensionPropertyRead),
    Write(ResolvedExtensionPropertyWrite),
}

pub(super) enum ExtensionPropertySelectionOutcome {
    NoCandidate,
    NoApplicable,
    Failed,
    Resolved(Box<SelectedExtensionProperty>),
}

impl Lowerer {
    pub(crate) fn resolve_extension_property_read(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> ExtensionPropertyResolution<ResolvedExtensionPropertyRead> {
        match self.resolve_extension_property_for(
            receiver,
            name,
            sink,
            ExtensionPropertyAccess::Read,
        ) {
            ExtensionPropertyResolution::Resolved(selected) => match *selected {
                SelectedExtensionProperty::Read(property) => {
                    ExtensionPropertyResolution::Resolved(Box::new(property))
                }
                SelectedExtensionProperty::Write(_) => {
                    unreachable!("read lookup produces a readable extension property")
                }
            },
            ExtensionPropertyResolution::NoCandidate => ExtensionPropertyResolution::NoCandidate,
            ExtensionPropertyResolution::Failed => ExtensionPropertyResolution::Failed,
        }
    }

    pub(crate) fn resolve_extension_property_write(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
    ) -> ExtensionPropertyResolution<ResolvedExtensionPropertyWrite> {
        match self.resolve_extension_property_for(
            receiver,
            name,
            sink,
            ExtensionPropertyAccess::Write,
        ) {
            ExtensionPropertyResolution::Resolved(selected) => match *selected {
                SelectedExtensionProperty::Write(property) => {
                    ExtensionPropertyResolution::Resolved(Box::new(property))
                }
                SelectedExtensionProperty::Read(_) => {
                    unreachable!("write lookup produces an extension property write target")
                }
            },
            ExtensionPropertyResolution::NoCandidate => ExtensionPropertyResolution::NoCandidate,
            ExtensionPropertyResolution::Failed => ExtensionPropertyResolution::Failed,
        }
    }

    fn resolve_extension_property_for(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        sink: &mut Vec<hir::Statement>,
        access: ExtensionPropertyAccess,
    ) -> ExtensionPropertyResolution<SelectedExtensionProperty> {
        let declared = self
            .top_level_namespaces
            .extension_property_layers(self.current_file, &name.text)
            .iter()
            .any(|properties| !properties.is_empty());
        let mut first_failure = None;
        for layer in self.named_extension_property_layers(&name.text) {
            if layer.candidates.is_empty() {
                continue;
            }
            let mut state = self.clone();
            let mut layer_sink = Vec::new();
            match state.select_extension_property_candidates(
                receiver.clone(),
                name,
                &layer.candidates,
                &mut layer_sink,
                access,
            ) {
                ExtensionPropertySelectionOutcome::Resolved(resolved) => {
                    *self = state;
                    sink.extend(layer_sink);
                    return ExtensionPropertyResolution::Resolved(resolved);
                }
                ExtensionPropertySelectionOutcome::NoApplicable => {
                    first_failure.get_or_insert(Box::new(state));
                }
                ExtensionPropertySelectionOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return ExtensionPropertyResolution::Failed;
                }
                ExtensionPropertySelectionOutcome::NoCandidate => {}
            }
        }
        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
            ExtensionPropertyResolution::Failed
        } else if declared {
            self.error(
                name.span,
                format!("extension property `{}` is not accessible here", name.text),
            );
            ExtensionPropertyResolution::Failed
        } else {
            ExtensionPropertyResolution::NoCandidate
        }
    }

    pub(crate) fn resolve_extension_property_candidates_outcome(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        properties: &[ExtensionPropertyTarget],
        sink: &mut Vec<hir::Statement>,
    ) -> ExtensionPropertyCandidateOutcome<ResolvedExtensionPropertyRead> {
        match self.select_extension_property_candidates(
            receiver,
            name,
            properties,
            sink,
            ExtensionPropertyAccess::Read,
        ) {
            ExtensionPropertySelectionOutcome::Resolved(selected) => match *selected {
                SelectedExtensionProperty::Read(property) => {
                    ExtensionPropertyCandidateOutcome::Resolved(Box::new(property))
                }
                SelectedExtensionProperty::Write(_) => {
                    unreachable!("read candidate lookup produces a readable property")
                }
            },
            ExtensionPropertySelectionOutcome::NoCandidate => {
                ExtensionPropertyCandidateOutcome::NoCandidate
            }
            ExtensionPropertySelectionOutcome::NoApplicable => {
                ExtensionPropertyCandidateOutcome::NoApplicable
            }
            ExtensionPropertySelectionOutcome::Failed => ExtensionPropertyCandidateOutcome::Failed,
        }
    }

    pub(crate) fn resolve_extension_property_write_candidates_outcome(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        properties: &[ExtensionPropertyTarget],
        sink: &mut Vec<hir::Statement>,
    ) -> ExtensionPropertyCandidateOutcome<ResolvedExtensionPropertyWrite> {
        match self.select_extension_property_candidates(
            receiver,
            name,
            properties,
            sink,
            ExtensionPropertyAccess::Write,
        ) {
            ExtensionPropertySelectionOutcome::Resolved(selected) => match *selected {
                SelectedExtensionProperty::Write(property) => {
                    ExtensionPropertyCandidateOutcome::Resolved(Box::new(property))
                }
                SelectedExtensionProperty::Read(_) => {
                    unreachable!("write candidate lookup produces a property write target")
                }
            },
            ExtensionPropertySelectionOutcome::NoCandidate => {
                ExtensionPropertyCandidateOutcome::NoCandidate
            }
            ExtensionPropertySelectionOutcome::NoApplicable => {
                ExtensionPropertyCandidateOutcome::NoApplicable
            }
            ExtensionPropertySelectionOutcome::Failed => ExtensionPropertyCandidateOutcome::Failed,
        }
    }
}
