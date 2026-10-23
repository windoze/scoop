use super::*;
use hir::{EditingMemberTarget as Target, EditingUnavailable as U, EditingValue as V};
use std::collections::BTreeSet;

mod extensions;
mod imported;

impl Lowerer {
    pub(super) fn editing_receiver_members(
        &mut self,
        receiver: &hir::Expr,
        safe_navigation: bool,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<V<hir::EditingMembers>, EditingCancelled> {
        match self.editing_receiver_types(receiver, safe_navigation) {
            V::Known(types) => self.editing_type_members(&types, is_cancelled),
            V::Unavailable(reason) => Ok(V::Unavailable(reason)),
        }
    }

    pub(super) fn editing_receiver_types(
        &mut self,
        receiver: &hir::Expr,
        safe_navigation: bool,
    ) -> V<Vec<hir::TypeId>> {
        V::Known(if safe_navigation {
            let Some(inner) = self.as_option(receiver.ty) else {
                return V::Unavailable(U::BodyFailure);
            };
            vec![inner]
        } else {
            self.smart_cast_receiver_views(receiver)
                .into_iter()
                .map(|view| view.ty)
                .collect()
        })
    }

    pub(super) fn editing_type_members(
        &mut self,
        types: &[hir::TypeId],
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<V<hir::EditingMembers>, EditingCancelled> {
        let mut members = hir::EditingMembers::default();
        for &ty in types {
            super::receivers::check_cancelled(is_cancelled)?;
            if self.resolve_imported_member_receiver_type(ty).is_err() {
                return Ok(V::Unavailable(U::DeclarationFailure));
            }
            for candidate in self.visible_methods(ty) {
                super::receivers::check_cancelled(is_cancelled)?;
                let function = candidate.function;
                if self.functions[function]
                    .method
                    .is_some_and(|method| method.availability.is_some())
                {
                    let arguments = self.callable_candidate_owner_arguments(&candidate);
                    if let Some(condition) = self.local_member_condition(function, &arguments)
                        && !self.conditional_application_holds(&condition)
                    {
                        continue;
                    }
                }
                let name = self.source_function_name(function).to_owned();
                if self.declaration_surface.rejects_function(function)
                    || self
                        .declaration_surface
                        .unavailable_signature(function)
                        .is_some()
                {
                    unavailable(&mut members, name, U::SignatureFailure);
                    continue;
                }
                let signature = &self.signatures[&function];
                if matches!(self.types[ty], hir::Type::Interface(_))
                    && signature.type_params.len() != signature.owner_type_param_count
                {
                    continue;
                }
                add(
                    &mut members,
                    name,
                    Target::Callable(hir::SemanticCallable::Current(function)),
                    false,
                );
            }
            let names = self
                .properties
                .iter()
                .map(|(_, property)| property.name.clone())
                .collect::<BTreeSet<_>>();
            for name in names {
                super::receivers::check_cancelled(is_cancelled)?;
                if let Some((property, _, _)) = self.find_accessible_nominal_property(ty, &name) {
                    add(&mut members, name, Target::Property(property), false);
                }
            }
            if self.editing_imported_members(ty, &mut members).is_err() {
                return Ok(V::Unavailable(U::DeclarationFailure));
            }
            self.editing_extensions(ty, &mut members, is_cancelled)?;
        }
        members.candidates.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then(a.is_extension.cmp(&b.is_extension))
        });
        members.unavailable.sort_by(|a, b| a.name.cmp(&b.name));
        members
            .unavailable
            .dedup_by(|a, b| a.name == b.name && a.reason == b.reason);
        Ok(V::Known(members))
    }

    fn editing_lookup_names(&self) -> BTreeSet<String> {
        let mut names = self.top_level_namespaces.lookup_names(self.current_file);
        let package = match self
            .top_level_namespaces
            .source_namespace(self.current_file)
        {
            crate::namespace::TopLevelLookupLayer::CurrentPackage(package) => Some(package),
            crate::namespace::TopLevelLookupLayer::CorePrelude => None,
        };
        names.extend(self.imports.lookup_names(self.current_file, package));
        names
    }
}

fn add(members: &mut hir::EditingMembers, name: String, target: Target, is_extension: bool) {
    if !members
        .candidates
        .iter()
        .any(|member| member.name == name && member.target == target)
    {
        members.candidates.push(hir::EditingMember {
            name,
            target,
            is_extension,
        });
    }
}

fn unavailable(members: &mut hir::EditingMembers, name: String, reason: U) {
    members
        .unavailable
        .push(hir::EditingUnavailableMember { name, reason });
}
