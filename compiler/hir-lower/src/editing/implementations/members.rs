use super::*;
use crate::editing::identities::DeclarationIdentities;
use hir::{EditingImplementationMember as Member, ImportedCallableSource};
use scoop_identity::CallableTemplateOrigin as Imported;

mod conformances;
mod imported;
mod selection;
mod source;

#[derive(Default)]
struct MemberIndex {
    result: hir::EditingMemberImplementationIndex,
    identities: DeclarationIdentities,
    relations: HashSet<hir::EditingMemberImplementationRelation>,
    implementations: HashSet<Member>,
    imported: HashSet<Imported>,
    imported_properties: HashSet<hir::PropertyDeclarationId>,
    imported_slots: Option<HashMap<scoop_identity::PersistentDispatchSlotId, Imported>>,
    functions: HashSet<hir::FunctionId>,
    properties: HashSet<hir::PropertyId>,
}

impl DeclarationAnalysis {
    pub fn member_implementation_index(
        &mut self,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<V<&hir::EditingMemberImplementationIndex>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        if self.member_implementations.is_none() {
            let mut builder = MemberIndex::default();
            for function in &self.functions {
                if is_cancelled() {
                    return Err(EditingCancelled);
                }
                builder.source_function(&mut self.lowerer, function.function, &is_cancelled)?;
            }
            let properties = self
                .lowerer
                .property_name_spans
                .keys()
                .copied()
                .collect::<Vec<_>>();
            for property in properties {
                if is_cancelled() {
                    return Err(EditingCancelled);
                }
                builder.source_property(&mut self.lowerer, property, &is_cancelled)?;
            }
            let owners = self
                .type_references
                .iter()
                .filter_map(|reference| {
                    if !reference.is_declaration {
                        return None;
                    }
                    let Identity::Nominal(declaration) = reference.target else {
                        return None;
                    };
                    self.lowerer.nominal_owners.get(&declaration).copied()
                })
                .collect::<Vec<_>>();
            for owner in owners {
                if is_cancelled() {
                    return Err(EditingCancelled);
                }
                builder.conformances(&mut self.lowerer, owner, &is_cancelled)?;
            }
            if is_cancelled() {
                return Err(EditingCancelled);
            }
            self.member_implementations = Some(builder.result);
        }
        Ok(V::Known(
            self.member_implementations
                .as_ref()
                .expect("complete member index"),
        ))
    }
}

impl MemberIndex {
    fn relation(&mut self, requirement: Member, implementation: Member) {
        let relation = hir::EditingMemberImplementationRelation {
            requirement,
            implementation,
        };
        if requirement != implementation && self.relations.insert(relation) {
            self.result.relations.push(relation);
        }
    }

    fn implementation(&mut self, declaration: Member, origin: Option<hir::SemanticSourceLocation>) {
        if let Some(origin) = origin
            && self.implementations.insert(declaration)
        {
            self.result
                .implementations
                .push(hir::EditingMemberImplementation {
                    declaration,
                    origin,
                });
        }
    }

    fn current_member(
        &mut self,
        lowerer: &mut Lowerer,
        function: hir::FunctionId,
    ) -> Option<Member> {
        lowerer.functions[function].method?;
        if let Some(property) = lowerer.function_property(function) {
            return self.current_property(lowerer, property);
        }
        match self
            .identities
            .callable(lowerer, hir::SemanticCallable::Current(function))?
        {
            hir::EditingCallableIdentity::Declaration(declaration) => {
                Some(Member::Callable(declaration))
            }
            hir::EditingCallableIdentity::Local(_)
            | hir::EditingCallableIdentity::Constructor(_)
            | hir::EditingCallableIdentity::Variant(_) => None,
        }
    }

    fn current_property(
        &mut self,
        lowerer: &mut Lowerer,
        property: hir::PropertyId,
    ) -> Option<Member> {
        if matches!(
            lowerer.properties[property].owner,
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_)
        ) {
            return None;
        }
        self.identities
            .property(lowerer, hir::SemanticProperty::Current(property))
            .map(Member::Property)
    }

    fn imported_member(&self, lowerer: &Lowerer, origin: Imported) -> Option<Member> {
        let dependencies = lowerer.dependencies.as_ref()?;
        let declaration = dependencies.callable_declaration(origin).ok()?;
        if !matches!(
            declaration.interface().owner(),
            hir::PublicDeclarationOwnerV1::Nominal(_)
        ) {
            return None;
        }
        Some(match origin {
            Imported::Function(id) => {
                Member::Callable(hir::DefaultCallableDeclarationV1::Function(id))
            }
            Imported::GenericFunction(id) => {
                Member::Callable(hir::DefaultCallableDeclarationV1::GenericFunction(id))
            }
            Imported::Accessor(id) => {
                Member::Property(dependencies.property_for_accessor(id)?.declaration())
            }
            Imported::Constructor(_) | Imported::VariantConstructor(_) => return None,
        })
    }
}
