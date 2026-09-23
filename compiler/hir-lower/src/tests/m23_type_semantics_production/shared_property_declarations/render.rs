use super::super::declaration_dump::{named, nominal, ty};
use super::*;

pub(super) fn table(
    properties: &Properties,
    callables: &Callables,
    identities: &ValidatedIdentityGraph,
) -> String {
    let mut rows = Vec::new();
    for property in properties.all_declarations() {
        let key = match property.declaration() {
            PropertyOwner::Property(id) => identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
            PropertyOwner::ExtensionProperty(id) => identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
        };
        let owner = match property.owner() {
            hir::PublicDeclarationOwnerV1::Nominal(owner) => nominal(owner, identities),
            other => format!("{other:?}"),
        };
        let property_name = format!("{owner}.{}", named(&key));
        rows.push(format!(
            "{property_name}:{} {:?} {:?} lookup={}\n",
            ty(property.value_type(), identities),
            property.declared_visibility(),
            property.representation(),
            properties.get(property.declaration()).is_some()
        ));
        for (id, role) in std::iter::once((property.accessors().getter(), AccessorRole::Getter))
            .chain(
                property
                    .accessors()
                    .setter()
                    .map(|id| (id, AccessorRole::Setter)),
            )
        {
            let id = CallableTemplateOrigin::Accessor(id);
            let record = callables.declaration(id).unwrap();
            let parameters = record
                .parameters()
                .parameters()
                .iter()
                .map(|p| format!("{}:{}", p.name().as_str(), ty(p.value_type(), identities)))
                .collect::<Vec<_>>()
                .join(",");
            rows.push(format!(
                "{property_name}.{role:?}({parameters})->{} {:?} {:?} lookup={} slots={}\n",
                ty(record.result(), identities),
                record.declared_visibility(),
                record.modality(),
                callables.get(id).is_some(),
                record.slot_relations().values().len()
            ));
        }
    }
    rows.sort();
    rows.concat()
}
