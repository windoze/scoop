use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, CborIdentityRecord,
    ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionAtomRole, DefinitionAtomSubkey,
    DefinitionOwnerChain, DependencyCallableDeclarationId, Effect, ExactCallableSignature,
    ExactTypeKey, GcEffect, LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PendingIdentityValidation, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, RuntimeIdentityRecord, ScoopAbiReturn, SourceDeclarationKey,
    SourceDeclarationSite, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole, ValidatedIdentityGraph,
};

use super::*;
use crate::{CanonicalLirFoundation, OdrFreeLirFoundation};

mod relations;
mod wire;

struct Fixture {
    producer: ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    target: StrongCallableDefinitionOwner,
    unit: PersistentExactTypeId,
    abi: CanonicalScoopAbiFunctionSignature,
    body: PersistentCallableBodyId,
    foundation: OdrFreeLirFoundation,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let coordinate = ConeCoordinate::new("test", "lir-bridge-provider", "1.0.0").unwrap();
        Self::for_producer(coordinate.identity().unwrap(), name)
    }

    fn for_producer(producer: ConeIdentity, name: &str) -> Self {
        let function = function(producer, name);
        let declaration = DependencyCallableDeclarationId::Function(function);
        let target = StrongCallableDefinitionOwner::Function(function);
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
        let abi = CanonicalScoopAbiFunctionSignature::new(
            ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
            Vec::new(),
            ScoopAbiReturn::unit_void(),
            GcEffect::NoGc,
        )
        .unwrap();
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target)).unwrap();
        let body_record =
            RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(target)).unwrap();
        let definition = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                producer,
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableBody,
            )
            .unwrap(),
        )
        .unwrap();
        let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            definition.id(),
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap();
        let symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::CallableBody(body),
            LinkageClass::ConeStrong,
        )
        .unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_callable_bodies(vec![body_record]).unwrap();
        canonical.set_definition_plans(vec![definition]).unwrap();
        canonical.set_definition_atoms(vec![atom]).unwrap();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
        let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();
        Self {
            producer,
            declaration,
            target,
            unit,
            abi,
            body,
            foundation,
        }
    }

    fn export(&self) -> ParamFreeLirCallableExportV1 {
        ParamFreeLirCallableExportV1::new(
            self.producer,
            self.declaration,
            self.target,
            self.abi.clone(),
            CallingConvention::Cdecl,
            DependencyExternalCallableRootPlanV1::NoGc,
        )
        .unwrap()
    }

    fn identities(&self, extra: &[DependencyCallableDeclarationId]) -> ValidatedIdentityGraph {
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(self.producer).unwrap();
        pending.register_authority(self.unit).unwrap();
        register_declaration(&mut pending, self.declaration);
        for declaration in extra {
            register_declaration(&mut pending, *declaration);
        }
        pending.finish().unwrap()
    }
}

fn register_declaration(
    pending: &mut PendingIdentityValidation<'_>,
    declaration: DependencyCallableDeclarationId,
) {
    match declaration {
        DependencyCallableDeclarationId::Function(id) => {
            pending.register_authority(id).unwrap();
        }
        DependencyCallableDeclarationId::PropertyAccessor(id) => {
            pending.register_authority(id).unwrap();
        }
    }
}

fn empty_foundation(producer: ConeIdentity) -> OdrFreeLirFoundation {
    OdrFreeLirFoundation::try_new(producer, CanonicalLirFoundation::empty()).unwrap()
}

fn function(producer: ConeIdentity, name: &str) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            producer,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn expected_definition(
    provider: ConeIdentity,
    body: PersistentCallableBodyId,
) -> ObjectDefinitionPlanId {
    ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(
            provider,
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn dependency_lir_selection_retains_consumer_and_canonical_lookup() {
    let fixture = Fixture::new("selectedDependency");
    let consumer = ConeCoordinate::new("test", "lir-bridge-consumer", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let selected = SelectedDependencyLirCallableV1::new(
        fixture.producer,
        fixture.declaration,
        fixture.target,
        fixture.abi.clone(),
        CallingConvention::Cdecl,
        DependencyExternalCallableRootPlanV1::NoGc,
    )
    .unwrap();
    let section = CrossConeLirBridgeSectionV1::try_new(
        &empty_foundation(consumer),
        Vec::new(),
        vec![selected.clone()],
    )
    .unwrap();
    let selection = SelectedDependencyLirSet::try_from_bridge(&section).unwrap();

    assert_eq!(section.artifact(), consumer);
    assert_eq!(selection.consumer(), consumer);
    assert_eq!(selection.callables(), &[selected]);
    let id = selection
        .callable_for(fixture.producer, fixture.declaration)
        .unwrap();
    assert_eq!(selection.callable(id), selection.callables().first());
}

#[test]
fn producer_side_lir_selection_is_canonical_and_closed() {
    let first = Fixture::new("firstSelection");
    let second = Fixture::new("secondSelection");
    let consumer = ConeCoordinate::new("test", "lir-selection-consumer", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let selected = |fixture: &Fixture, provider| {
        SelectedDependencyLirCallableV1::new(
            provider,
            fixture.declaration,
            fixture.target,
            fixture.abi.clone(),
            CallingConvention::Cdecl,
            DependencyExternalCallableRootPlanV1::NoGc,
        )
        .unwrap()
    };
    let mut expected = vec![
        selected(&first, first.producer),
        selected(&second, second.producer),
    ];
    expected
        .sort_unstable_by_key(|callable| (callable.provider(), callable.bridge().declaration()));

    let selection = SelectedDependencyLirSet::try_from_callables(
        consumer,
        expected.iter().cloned().rev().collect(),
    )
    .unwrap();
    assert_eq!(selection.callables(), expected);

    let duplicate = selected(&first, first.producer);
    assert!(matches!(
        SelectedDependencyLirSet::try_from_callables(consumer, vec![duplicate.clone(), duplicate],),
        Err(SelectedDependencyLirSetBuildError::DuplicateCallable { .. })
    ));

    for (provider, expected) in [
        (
            consumer,
            SelectedDependencyLirSetBuildError::SelectedCurrentProvider { provider: consumer },
        ),
        (
            ConeIdentity::CORE,
            SelectedDependencyLirSetBuildError::SelectedTrustedCore,
        ),
    ] {
        assert_eq!(
            SelectedDependencyLirSet::try_from_callables(
                consumer,
                vec![selected(&first, provider)],
            )
            .err(),
            Some(expected)
        );
    }
}
