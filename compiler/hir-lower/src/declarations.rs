use super::*;
use crate::persistent_nominals::NominalIdentityInput;
use scoop_identity::SourceNominalKind;

mod callables;
mod class;
mod enumeration;
mod interface;
mod nested;
mod objects;
mod structure;
pub(crate) use nested::NestedDeclarationQueues;
pub(crate) use objects::ObjectSource;

impl Lowerer {
    pub(super) fn require_core_struct(
        &mut self,
        name: &str,
        files: &[ast::SourceFile],
    ) -> Option<StructId> {
        let candidate = self
            .core_nominal_target(name)
            .and_then(|target| match target {
                NominalTarget::Struct(id) => Some(id),
                _ => None,
            });
        if let Some(id) = candidate {
            return Some(id);
        }
        let core_diagnostic_file = self.core_diagnostic_file();
        self.current_file = core_diagnostic_file;
        self.error(
            files[core_diagnostic_file].span,
            format!("scoop.core must define exactly one `{name}` struct"),
        );
        None
    }

    /// Whether a name is already taken in the shared type namespace. This
    /// includes compiler built-ins, top-level aliases and every nominal kind.
    /// Returns the kind of the existing declaration for diagnostics.
    pub(super) fn type_namespace_conflict(
        &self,
        owner: Option<Owner>,
        name: &str,
        file: usize,
        file_private: bool,
    ) -> Option<&'static str> {
        if let Some(target) = owner.and_then(|owner| {
            self.nested_nominals_by_owner
                .get(&(owner, name.to_string()))
        }) {
            return Some(match target {
                NominalTarget::Struct(_) => "a struct",
                NominalTarget::Enum(_) => "an enum",
                NominalTarget::Class(_) => "a class",
                NominalTarget::Interface(_) => "an interface",
                NominalTarget::Object(_) => "an object",
            });
        }
        if owner.is_some() {
            return None;
        }
        if matches!(name, "Unit" | "Any") {
            Some("a built-in type")
        } else {
            self.top_level_namespaces
                .type_conflict(file, name, file_private)
                .map(crate::namespace::TopLevelTypeTarget::description)
        }
    }
}
