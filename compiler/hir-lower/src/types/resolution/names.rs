//! Source and lexical type name lookup.

use super::*;
use crate::{NominalTarget, Owner};

impl Lowerer {
    pub(crate) fn top_level_type_target_is_accessible(
        &self,
        target: crate::namespace::TopLevelTypeTarget,
    ) -> bool {
        let domain = match target {
            crate::namespace::TopLevelTypeTarget::Annotation(id) => {
                &self.source_annotations[&id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Alias(alias) => {
                return self.source_type_alias_is_accessible(alias);
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Struct(id)) => {
                &self.structs[id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Enum(id)) => {
                &self.enums[id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Class(id)) => {
                &self.classes[id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Interface(id)) => {
                &self.interfaces[id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Object(id)) => {
                &self.objects[id].access.lookup.0
            }
        };
        self.access_domain_allows(domain)
    }

    pub(crate) fn top_level_type_target(
        &self,
        name: &str,
    ) -> Option<crate::namespace::TopLevelTypeTarget> {
        match self.lookup_type(name) {
            crate::imports::lookup::LookupResult::Unique(candidate) => candidate.target.current(),
            crate::imports::lookup::LookupResult::Missing
            | crate::imports::lookup::LookupResult::Ambiguous { .. }
            | crate::imports::lookup::LookupResult::Inaccessible(_) => None,
        }
    }

    pub(crate) fn top_level_type_target_for_reference(
        &self,
        name: &str,
    ) -> Option<crate::namespace::TopLevelTypeTarget> {
        match self.lookup_type(name) {
            crate::imports::lookup::LookupResult::Unique(candidate) => candidate.target.current(),
            crate::imports::lookup::LookupResult::Inaccessible(candidates)
                if candidates.len() == 1 =>
            {
                candidates.first().target.current()
            }
            crate::imports::lookup::LookupResult::Missing
            | crate::imports::lookup::LookupResult::Ambiguous { .. }
            | crate::imports::lookup::LookupResult::Inaccessible(_) => None,
        }
    }

    pub(crate) fn top_level_nominal_target(&self, name: &str) -> Option<NominalTarget> {
        match self.top_level_type_target_for_reference(name)? {
            crate::namespace::TopLevelTypeTarget::Nominal(target) => Some(target),
            crate::namespace::TopLevelTypeTarget::Alias(_)
            | crate::namespace::TopLevelTypeTarget::Annotation(_) => None,
        }
    }

    pub(crate) fn core_nominal_target(&self, name: &str) -> Option<NominalTarget> {
        let mut declarations = self
            .intrinsic_sources
            .iter()
            .enumerate()
            .filter(|(_, source)| source.kind == crate::SourceKind::Core)
            .flat_map(|(file, _)| self.top_level_namespaces.declared_types_in_file(file, name));
        let target = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        match target {
            crate::namespace::TopLevelTypeTarget::Nominal(target) => Some(target),
            crate::namespace::TopLevelTypeTarget::Alias(_)
            | crate::namespace::TopLevelTypeTarget::Annotation(_) => None,
        }
    }

    pub(crate) fn top_level_struct_named(&self, name: &str) -> Option<(StructId, TypeId)> {
        let NominalTarget::Struct(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        let ty = self.struct_applications[self.structs[id].self_application].canonical_type;
        Some((id, ty))
    }

    pub(crate) fn top_level_enum_named(&self, name: &str) -> Option<hir::EnumId> {
        let NominalTarget::Enum(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        Some(id)
    }

    pub(crate) fn top_level_class_named(&self, name: &str) -> Option<(hir::ClassId, TypeId)> {
        let NominalTarget::Class(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        let ty = self.class_applications[self.classes[id].self_application].canonical_type;
        Some((id, ty))
    }

    pub(crate) fn top_level_interface_named(
        &self,
        name: &str,
    ) -> Option<(hir::InterfaceId, TypeId)> {
        let NominalTarget::Interface(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        let ty = self.interface_applications[self.interfaces[id].self_application].canonical_type;
        Some((id, ty))
    }

    pub(crate) fn top_level_object_named(&self, name: &str) -> Option<hir::ObjectId> {
        let NominalTarget::Object(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        Some(id)
    }

    pub(crate) fn nested_nominal_target(&self, owner: Owner, name: &str) -> Option<NominalTarget> {
        self.nested_nominals_by_owner
            .get(&(owner, name.to_string()))
            .copied()
    }

    pub(crate) fn nominal_parent(&self, owner: Owner) -> Option<Owner> {
        let parent = match owner {
            Owner::Class(id) => self.classes[id].owner,
            Owner::Interface(id) => self.interfaces[id].owner,
            Owner::Struct(id) => self.structs[id].owner,
            Owner::Enum(id) => self.enums[id].owner,
            Owner::Object(id) => self.objects[id].owner,
        }?;
        Some(Owner::from_nominal_owner(parent))
    }

    pub(crate) fn lexical_nested_nominal_target(&self, name: &str) -> Option<NominalTarget> {
        if self.scopes.type_alias(name).is_some() {
            return None;
        }
        let mut owner = self.current_owner?;
        loop {
            if self
                .nested_aliases_by_owner
                .contains_key(&(owner, name.to_owned()))
            {
                return None;
            }
            if let Some(target) = self
                .nested_nominals_by_owner
                .get(&(owner, name.to_string()))
            {
                return Some(*target);
            }
            owner = self.nominal_parent(owner)?;
        }
    }

    pub(super) fn outer_type_parameter_owner(&self, name: &str) -> Option<Owner> {
        let mut owner = self.nominal_parent(self.current_owner?)?;
        loop {
            let params: &[hir::TypeParamDecl] = match owner {
                Owner::Class(id) => &self.classes[id].type_params,
                Owner::Interface(id) => &self.interfaces[id].type_params,
                Owner::Struct(id) => &self.structs[id].type_params,
                Owner::Enum(id) => &self.enums[id].type_params,
                Owner::Object(_) => &[],
            };
            if params.iter().any(|parameter| parameter.name == name) {
                return Some(owner);
            }
            owner = self.nominal_parent(owner)?;
        }
    }
}
