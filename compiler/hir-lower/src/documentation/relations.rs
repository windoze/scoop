//! Browse the same receiver hierarchy used by ordinary member lookup.

use super::*;
use hir::{
    DocumentationRelation as Relation, DocumentationRelationKind as RelationKind,
    DocumentationSymbol as Symbol, ImportedCallableSource,
};

mod conditions;
mod constraints;
mod implemented;
mod imported;
mod methods;
mod overrides;
mod properties;
use properties::{imported_visibility, property_signature};

impl Lowerer {
    pub(super) fn documentation_relations(&mut self, target: Target) -> Vec<Relation> {
        match target {
            Target::Declaration(Source::Nominal(owner)) => self.documentation_inheritance(owner),
            Target::Declaration(Source::Callable(Callable::Function(function))) => {
                self.documentation_overrides(function)
            }
            Target::Declaration(Source::Property(property)) => self.properties[property]
                .overrides
                .clone()
                .into_iter()
                .map(|reference| self.documentation_property_override(reference))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn documentation_inheritance(&mut self, nominal: hir::NominalOwner) -> Vec<Relation> {
        let owner = Owner::from_nominal_owner(nominal);
        let ty = self.owner_ty(owner);
        self.current_owner = Some(owner);
        let mut relations = Vec::new();
        let parents = self.direct_nominal_supertypes(ty);
        let conditional = self.owner_conditional_interfaces(owner).to_vec();
        for parent in &parents {
            if conditional
                .iter()
                .any(|declaration| declaration.interface == *parent)
            {
                continue;
            }
            if let Some(target) = self.documentation_nominal_symbol(*parent) {
                relations.push(Relation {
                    kind: RelationKind::Supertype,
                    target,
                    signature: self.type_name(*parent),
                    origin: String::new(),
                    visibility: hir::DeclaredVisibility::Public,
                    hidden: false,
                });
            }
        }
        for declaration in conditional {
            let condition = self.conditional_guard_application(hir::ConditionalInterfaceGuard {
                declaration: declaration.id(),
                owner: ty,
            });
            if let Some(target) = self.documentation_nominal_symbol(declaration.interface) {
                let suffix = self.documentation_condition(&condition);
                relations.push(Relation {
                    kind: RelationKind::Supertype,
                    target,
                    signature: format!(
                        "{}{suffix}",
                        self.type_name_with_params(
                            declaration.interface,
                            &self.type_params_in_scope
                        )
                    ),
                    origin: String::new(),
                    visibility: hir::DeclaredVisibility::Public,
                    hidden: false,
                });
            }
        }
        let inherited = self.documentation_inherited_members(owner, ty);
        let ordinary = inherited
            .iter()
            .map(|(relation, _)| (relation.target, relation.signature.clone()))
            .collect::<HashSet<_>>();
        relations.extend(inherited.into_iter().map(|(relation, _)| relation));
        for condition in self.documentation_inherited_conditions(ty) {
            if self.conditional_application_is_impossible(&condition) {
                continue;
            }
            let suffix = self.documentation_condition(&condition);
            let additions = self.with_conditional_interface_assumptions(&condition, |this| {
                this.documentation_inherited_members(owner, ty)
            });
            for (mut relation, guarded) in additions {
                if ordinary.contains(&(relation.target, relation.signature.clone())) {
                    continue;
                }
                if !guarded {
                    relation.signature.push_str(&suffix);
                }
                relations.push(relation);
            }
        }
        let mut seen = HashSet::new();
        relations.retain(|entry| seen.insert((entry.target, entry.signature.clone())));
        relations
    }

    fn documentation_inherited_members(
        &mut self,
        owner: Owner,
        ty: hir::TypeId,
    ) -> Vec<(Relation, bool)> {
        let mut relations = Vec::new();
        for method in self.visible_methods(ty) {
            if self.function_owner.get(&method.function) == Some(&owner) {
                continue;
            }
            let arguments = self.callable_candidate_owner_arguments(&method);
            let condition = self.local_member_condition(method.function, &arguments);
            if let Some(condition) = condition {
                if self.conditional_application_is_impossible(&condition) {
                    continue;
                }
                let suffix = self.documentation_condition(&condition);
                let mut relation = self
                    .with_conditional_interface_assumptions(&condition, |this| {
                        this.documentation_inherited_method(method)
                    });
                relation.signature.push_str(&suffix);
                relations.push((relation, true));
            } else {
                relations.push((self.documentation_inherited_method(method), false));
            }
        }
        let mut names = std::collections::BTreeSet::new();
        let mut pending = self.direct_nominal_supertypes(ty);
        let mut seen = HashSet::new();
        while let Some(parent) = pending.pop() {
            if !seen.insert(parent) {
                continue;
            }
            if let Some(target) = self.nominal_target_for_type(parent) {
                let properties = match target.owner() {
                    Owner::Class(id) => &self.classes[id].properties,
                    Owner::Struct(id) => &self.structs[id].properties,
                    Owner::Enum(id) => &self.enums[id].properties,
                    Owner::Interface(id) => &self.interfaces[id].properties,
                    Owner::Object(id) => &self.classes[self.objects[id].backing_class].properties,
                };
                names.extend(
                    properties
                        .iter()
                        .map(|id| (self.properties[*id].name.clone(), true)),
                );
            }
            if let Some(owner) = self.imported_nominal_owner(parent)
                && let Some(dependencies) = &self.dependencies
            {
                names.extend(dependencies.nominal_member_names(owner));
            }
            pending.extend(self.direct_nominal_supertypes(parent));
        }
        for (name, property) in names {
            if property
                && let Some((id, application, value)) =
                    self.find_accessible_nominal_property(ty, &name)
            {
                let declaring = self.method_owner_type(application);
                if self.documentation_nominal_symbol(declaring)
                    != Some(Symbol::Current(
                        Source::Nominal(owner.as_nominal_owner()).into(),
                    ))
                {
                    let condition = self.documentation_property_condition(id, application);
                    let suffix = condition
                        .as_ref()
                        .map(|condition| self.documentation_condition(condition))
                        .unwrap_or_default();
                    let property = &self.properties[id];
                    let mut relation = Relation {
                        kind: RelationKind::Inherited,
                        target: Symbol::Current(Source::Property(id).into()),
                        signature: property_signature(
                            &name,
                            &self.type_name(value),
                            self.property_getters[property.capability.getter()]
                                .access
                                .declared,
                            property
                                .capability
                                .setter()
                                .map(|setter| self.property_setters[setter].access.declared),
                        ),
                        origin: self.type_name(declaring),
                        visibility: property.access.declared,
                        hidden: false,
                    };
                    relation.signature.push_str(&suffix);
                    relations.push((relation, condition.is_some()));
                }
            }
            let lookup = if property {
                hir::ImportedMemberLookup::PropertyGetter(&name)
            } else {
                hir::ImportedMemberLookup::Name(&name)
            };
            let candidates = self
                .imported_member_candidates(ty, lookup)
                .expect("a completed HIR has valid imported member declarations");
            for candidate in candidates {
                let condition = self.imported_member_condition(candidate.interface(), ty);
                if let Some(condition) = condition {
                    if self.conditional_application_is_impossible(&condition) {
                        continue;
                    }
                    let suffix = self.documentation_condition(&condition);
                    let mut relation = self
                        .with_conditional_interface_assumptions(&condition, |this| {
                            this.documentation_imported_member(ty, candidate, property)
                        });
                    relation.signature.push_str(&suffix);
                    relations.push((relation, true));
                } else {
                    relations.push((
                        self.documentation_imported_member(ty, candidate, property),
                        false,
                    ));
                }
            }
        }
        relations
    }

    fn documentation_nominal_symbol(&self, ty: hir::TypeId) -> Option<Symbol> {
        if let Some(owner) = self.nominal_target_for_type(ty) {
            Some(Symbol::Current(
                Source::Nominal(owner.owner().as_nominal_owner()).into(),
            ))
        } else {
            self.imported_nominal_owner(ty)
                .map(|owner| Symbol::Imported(hir::AnnotationTargetV1::Nominal(owner)))
        }
    }
}
