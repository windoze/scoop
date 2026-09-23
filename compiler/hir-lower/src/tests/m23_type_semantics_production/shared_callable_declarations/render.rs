use super::*;
use scoop_identity::{
    DeclarationName, EnumVariantIdentityKey, SignatureTypeKey, SourceDeclarationKey,
    ValidatedIdentityGraph,
};

pub(super) fn table(table: &Table, identities: &ValidatedIdentityGraph) -> String {
    let mut rows = Vec::new();
    for record in table.all_declarations() {
        let declaration = record.declaration();
        let name = match declaration {
            CallableTemplateOrigin::Function(id) => named(
                &identities
                    .canonical_key::<_, SourceDeclarationKey>(id)
                    .unwrap(),
            ),
            CallableTemplateOrigin::GenericFunction(id) => named(
                &identities
                    .canonical_key::<_, SourceDeclarationKey>(id)
                    .unwrap(),
            ),
            CallableTemplateOrigin::Constructor(_) => "constructor".to_owned(),
            CallableTemplateOrigin::VariantConstructor(id) => identities
                .canonical_key::<_, EnumVariantIdentityKey>(id)
                .unwrap()
                .source_name()
                .unwrap()
                .as_str()
                .to_owned(),
            CallableTemplateOrigin::Accessor(_) => panic!("fixture has no public properties"),
        };
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = record.owner() else {
            panic!("nominal")
        };
        let parameters = record
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| {
                format!(
                    "{}:{}",
                    parameter.name().as_str(),
                    ty(parameter.value_type(), identities)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut bounds = Vec::new();
        for binder in record.type_parameters().binders() {
            let bound = match binder.bounds() {
                hir::TypeParameterBoundsV1::Nominal(bounds) => bounds
                    .class()
                    .into_iter()
                    .chain(bounds.interfaces().values())
                    .map(|b| ty(b, identities))
                    .collect::<Vec<_>>()
                    .join("&"),
                other => format!("{other:?}"),
            };
            bounds.push(format!("{}:{bound}", binder.name().as_str()));
        }
        rows.push(format!(
            "{}.{name}<{}>({parameters})->{} {:?} {:?} {:?} lookup={} slots={}\n",
            nominal(owner, identities),
            bounds.join(","),
            ty(record.result(), identities),
            record.declared_visibility(),
            record.modality(),
            record.effects(),
            table.get(declaration).is_some(),
            record.slot_relations().values().len()
        ));
    }
    rows.sort();
    rows.concat()
}

fn named(key: &SourceDeclarationKey) -> String {
    let DeclarationName::Named(name) = key.name() else {
        panic!("named source")
    };
    name.as_str().to_owned()
}

fn nominal(owner: hir::SourceNominalId, identities: &ValidatedIdentityGraph) -> String {
    match owner {
        hir::SourceNominalId::Concrete(id) => named(
            &identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
        ),
        hir::SourceNominalId::GenericTemplate(id) => named(
            &identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
        ),
    }
}

fn ty(value: &SignatureTypeKey, identities: &ValidatedIdentityGraph) -> String {
    match value {
        SignatureTypeKey::Nominal(id) => nominal(hir::SourceNominalId::Concrete(*id), identities),
        SignatureTypeKey::NominalApplication { origin, arguments } => format!(
            "{}<{}>",
            nominal(hir::SourceNominalId::GenericTemplate(*origin), identities),
            arguments
                .as_slice()
                .iter()
                .map(|v| ty(v, identities))
                .collect::<Vec<_>>()
                .join(",")
        ),
        SignatureTypeKey::Binder { depth, index } => format!("binder({depth},{index})"),
        other => panic!("unexpected fixture signature {other:?}"),
    }
}
