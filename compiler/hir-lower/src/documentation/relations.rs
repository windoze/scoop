//! Browse the same receiver hierarchy used by ordinary member lookup.

use super::*;
use hir::{
    DocumentationRelation as Relation, DocumentationRelationKind as RelationKind,
    DocumentationSymbol as Symbol, ImportedCallableSource,
};

mod constraints;
mod implemented;
mod imported;
mod methods;
mod overrides;

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
        for parent in &parents {
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
        for method in self.visible_methods(ty) {
            if self.function_owner.get(&method.function) != Some(&owner) {
                relations.push(self.documentation_inherited_method(method));
            }
        }
        let mut names = std::collections::BTreeSet::new();
        let mut pending = parents;
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
                    != Some(Symbol::Current(Source::Nominal(nominal).into()))
                {
                    let property = &self.properties[id];
                    relations.push(Relation {
                        kind: RelationKind::Inherited,
                        target: Symbol::Current(Source::Property(id).into()),
                        signature: format!(
                            "{} {name}: {}",
                            if property.capability.setter().is_some() {
                                "var"
                            } else {
                                "val"
                            },
                            self.type_name(value)
                        ),
                        origin: self.type_name(declaring),
                        visibility: property.access.declared,
                        hidden: false,
                    });
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
                relations.push(self.documentation_imported_member(ty, candidate, property));
            }
        }
        let mut seen = HashSet::new();
        relations.retain(|entry| seen.insert(entry.target));
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
