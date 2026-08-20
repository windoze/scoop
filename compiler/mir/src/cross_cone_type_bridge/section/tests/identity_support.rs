use super::*;
use scoop_hir::{CanonicalHirFoundation, DecodedHirFoundation};
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerChain,
    ExactCallableSignature, GeneratedCallableKey, InitializationUnitKey, PackagePath,
    PendingIdentityValidation, PersistentFunctionId, PersistentPropertyId, SourceDeclarationKey,
    SourceDeclarationSite,
};

pub(super) fn site(provider: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
pub(super) fn update(
    fixture: &mut TypeFixture,
    hir: CanonicalHirFoundation,
    mir: crate::CanonicalMirFoundation,
    external: &[&ValidatedIdentityGraph],
) {
    let hir: DecodedHirFoundation = decode_canonical(&encode(&hir).unwrap()).unwrap();
    let mir: crate::DecodedMirFoundation = decode_canonical(&encode(&mir).unwrap()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    hir.register_identities(&mut pending).unwrap();
    mir.register_identities(&mut pending).unwrap();
    pending
        .register_external_graph_authorities(&fixture.graph)
        .unwrap();
    for graph in external {
        pending.register_external_graph_authorities(graph).unwrap();
    }
    hir.resolve_identities(&mut pending).unwrap();
    mir.resolve_identities(&mut pending).unwrap();
    let mut graph = pending.finish().unwrap();
    fixture.foundation =
        crate::OdrFreeMirFoundation::from_validated(mir.validate(&mut graph).unwrap()).unwrap();
    fixture.graph = graph;
}
pub(super) fn add_function(
    fixture: &mut Fixture,
    name: &str,
) -> (PersistentFunctionId, ExactCallableSignature) {
    add_function_to(&mut fixture.types, fixture.provider, name)
}
pub(super) fn add_function_to(
    fixture: &mut TypeFixture,
    provider: ConeIdentity,
    name: &str,
) -> (PersistentFunctionId, ExactCallableSignature) {
    let function: CborIdentityRecord<PersistentFunctionId, _> =
        CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site(provider),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            vec![],
        ))
        .unwrap();
    let signature = ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        vec![],
        fixture.payload.id(),
    );
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_functions(vec![function.clone()]).unwrap();
    let mut mir = fixture.foundation.as_canonical().clone();
    let mut signatures = mir.callable_signatures().to_vec();
    signatures.push(crate::CallableSignatureRecord::new(
        crate::CallableSignatureSubject::strong(scoop_identity::CallableOwner::Function(
            function.id(),
        )),
        signature.clone(),
    ));
    mir.set_callable_signatures(signatures).unwrap();
    update(fixture, hir, mir, &[]);
    (function.id(), signature)
}
pub(super) fn add_units(fixture: &mut Fixture, core: &Fixture, names: &[&str]) {
    let properties: Vec<CborIdentityRecord<PersistentPropertyId, _>> = names
        .iter()
        .map(|name| {
            CborIdentityRecord::from_key(SourceDeclarationKey::property(
                site(fixture.provider),
                CanonicalIdentifier::new(name).unwrap(),
            ))
            .unwrap()
        })
        .collect();
    let units: Vec<crate::InitializationUnitIdentityRecord> = properties
        .iter()
        .map(|property| {
            CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(property.id()))
                .unwrap()
        })
        .collect();
    let generated: Vec<crate::GeneratedCallableRecord> = units
        .iter()
        .flat_map(|unit| {
            [
                InitializationCallableRole::Initializer,
                InitializationCallableRole::Ensure,
            ]
            .map(|role| {
                CborIdentityRecord::from_key(GeneratedCallableKey::Initialization {
                    unit: unit.id(),
                    role,
                })
                .unwrap()
            })
        })
        .collect();
    let signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        vec![],
        core.types.payload.id(),
    );
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_properties(properties).unwrap();
    hir.set_initialization_units(units.clone()).unwrap();
    let mut mir = fixture.types.foundation.as_canonical().clone();
    mir.set_generated_callables(generated.clone()).unwrap();
    mir.set_callable_signatures(
        generated
            .iter()
            .map(|record| {
                crate::CallableSignatureRecord::new(
                    crate::CallableSignatureSubject::strong(
                        scoop_identity::CallableOwner::Generated(record.id()),
                    ),
                    signature.clone(),
                )
            })
            .collect(),
    )
    .unwrap();
    update(&mut fixture.types, hir, mir, &[&core.types.graph]);
    fixture.production = production(fixture.provider, &fixture.types.foundation);
    let signature = MirBridgeCallableSignatureV1::new(signature, crate::GcEffect::Managed);
    fixture.units = units
        .iter()
        .map(|unit| {
            let callable = |role| {
                scoop_identity::PersistentGeneratedCallableId::from_key(
                    &GeneratedCallableKey::Initialization {
                        unit: unit.id(),
                        role,
                    },
                )
                .unwrap()
            };
            MirTypeBridgeInitializationUnitV1::new(
                unit.id(),
                callable(InitializationCallableRole::Initializer),
                callable(InitializationCallableRole::Ensure),
                signature.clone(),
            )
        })
        .collect();
    fixture
        .units
        .sort_unstable_by_key(MirTypeBridgeInitializationUnitV1::unit);
}
pub(super) fn set_uses(fixture: &mut Fixture, records: Vec<SelectedExternalInitializationUseV1>) {
    let source = &fixture.exports;
    fixture.exports = MirTypeBridgeExportConstituentsV1::new(
        source.types().clone(),
        source.callables().clone(),
        source.dispatch().clone(),
        source.objects().clone(),
        source.shapes().clone(),
        CanonicalMirExternalInitializationUsesV1::try_new(records).unwrap(),
    );
}
