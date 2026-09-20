use super::*;

mod accessors;
mod origins;
mod signatures;

fn check(
    fixture: &Fixture,
    sources: &Sources,
    core: &hir::ImportedCoreFundamentalTypeProtocol,
) -> Result<(), Error> {
    let foundation = fixture.bind().unwrap();
    let nominals = foundation
        .bind_nominal_sources(&sources.nominals, &mut meter())
        .unwrap();
    nominals
        .bind_member_sources(&sources.properties, &sources.callables, core, &mut meter())
        .map(|_| ())
}
fn replace_callable(sources: &mut Sources, record: hir::NominalSupportCallableInterfaceV1) {
    sources.callables = Callables::try_new(
        sources
            .callables
            .records()
            .iter()
            .map(|r| {
                if r.declaration() == record.declaration() {
                    record.clone()
                } else {
                    r.clone()
                }
            })
            .collect(),
        &mut meter(),
    )
    .unwrap();
}
fn replace_property(sources: &mut Sources, record: hir::NominalSupportPropertyInterfaceV1) {
    sources.properties = Properties::try_new(
        sources
            .properties
            .records()
            .iter()
            .map(|r| {
                if r.declaration() == record.declaration() {
                    record.clone()
                } else {
                    r.clone()
                }
            })
            .collect(),
        &mut meter(),
    )
    .unwrap();
}
fn callable(
    record: &hir::NominalSupportCallableInterfaceV1,
    parameters: hir::CanonicalSourceParameterShapesV1,
    result: SignatureTypeKey,
) -> hir::NominalSupportCallableInterfaceV1 {
    let p = record.payload();
    hir::NominalSupportCallableInterfaceV1::try_new(
        record.declaration(),
        record.declaration_access().clone(),
        hir::NominalSourceCallablePayloadV1::try_new(
            record.declaration(),
            p.owner(),
            p.type_parameters().clone(),
            parameters,
            result,
            p.effects(),
            p.modality(),
            p.slot_relations().clone(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn runtime(
    record: &hir::NominalSupportPropertyInterfaceV1,
) -> &hir::NominalSourcePropertyPayloadV1 {
    let hir::NominalSupportPropertyPayloadV1::Runtime { interface } = record.payload() else {
        panic!("runtime property");
    };
    interface
}
fn property(
    sources: &Sources,
    output: &hir::OrdinaryHirOutput<'_>,
    name: &str,
) -> hir::NominalSupportPropertyInterfaceV1 {
    let export = output.output().export.module();
    let (id, _) = export
        .properties
        .iter()
        .find(|(_, p)| p.name == name)
        .unwrap();
    let hir::HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
        panic!("ordinary property");
    };
    sources.properties.get(identity.id()).unwrap().clone()
}
fn access(
    source: &hir::DeclarationAccessSourceV1,
    visibility: hir::DeclaredVisibilityV1,
) -> hir::DeclarationAccessSourceV1 {
    hir::DeclarationAccessSourceV1::try_new(
        visibility,
        source.lexical_owners().to_vec(),
        source.definition_origin().clone(),
    )
    .unwrap()
}
fn with_mutability(
    record: &hir::NominalSupportPropertyInterfaceV1,
    mutability: hir::ProtectedPropertyMutabilityV1,
) -> hir::NominalSupportPropertyInterfaceV1 {
    let p = runtime(record);
    hir::NominalSupportPropertyInterfaceV1::try_new(
        record.declaration(),
        record.declaration_access().clone(),
        hir::NominalSupportPropertyPayloadV1::Runtime {
            interface: hir::NominalSourcePropertyPayloadV1::try_new(
                p.owner(),
                p.value_type().clone(),
                p.getter(),
                mutability,
                p.representation(),
                p.slot_relations().clone(),
            )
            .unwrap(),
        },
    )
    .unwrap()
}
