use std::collections::BTreeSet;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, ObjectDefinitionPlanOwner, OdrMemberKey,
    PersistentCallableBodyId, PropertyAccessorKey, PropertyOwner, SourceDeclarationKey,
    SpecializationKey, ValidatedIdentityGraph,
};

pub(super) fn core_bodies(
    lir: &scoop_lir::ConeLirOutput,
    identities: &ValidatedIdentityGraph,
) -> BTreeSet<PersistentCallableBodyId> {
    lir.module()
        .functions
        .iter()
        .filter_map(|function| {
            let body = &function.callable_body;
            let ObjectDefinitionPlanOwner::Odr { member } =
                body.definition_plan_key(lir.module().cone).owner()
            else {
                return None;
            };
            let member = identities.canonical_key::<_, OdrMemberKey>(member).unwrap();
            let group = identities
                .canonical_key::<_, SpecializationKey>(member.group())
                .unwrap();
            let declaration = match &*group {
                SpecializationKey::Nominal { origin, .. } => identities
                    .canonical_key::<_, SourceDeclarationKey>(*origin)
                    .unwrap(),
                SpecializationKey::Callable { application } => match application.origin() {
                    CallableTemplateOrigin::Function(id) => identities
                        .canonical_key::<_, SourceDeclarationKey>(id)
                        .unwrap(),
                    CallableTemplateOrigin::GenericFunction(id) => identities
                        .canonical_key::<_, SourceDeclarationKey>(id)
                        .unwrap(),
                    CallableTemplateOrigin::Constructor(id) => identities
                        .canonical_key::<_, SourceDeclarationKey>(id)
                        .unwrap(),
                    CallableTemplateOrigin::Accessor(id) => {
                        let accessor = identities
                            .canonical_key::<_, PropertyAccessorKey>(id)
                            .unwrap();
                        let PropertyOwner::Property(property) = accessor.owner() else {
                            panic!("the fixture uses only ordinary properties");
                        };
                        identities
                            .canonical_key::<_, SourceDeclarationKey>(property)
                            .unwrap()
                    }
                    origin => panic!("unexpected callable origin in this fixture: {origin:?}"),
                },
                group => panic!("unexpected specialization in this fixture: {group:?}"),
            };
            (declaration.origin() == ConeIdentity::CORE).then_some(body.id())
        })
        .collect()
}
