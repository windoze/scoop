use super::*;

pub(super) struct Fixture {
    foundation: ImportedMirFoundation,
    production: CoreBootstrapBridgeSectionV1,
    pub(super) cycle: PersistentFunctionId,
    ordinary: PersistentFunctionId,
    signature: ExactCallableSignature,
}

impl Fixture {
    pub(super) fn new() -> Self {
        Self::at(ConeIdentity::CORE)
    }

    pub(super) fn at(provider: ConeIdentity) -> Self {
        let function = |name| {
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    provider,
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
        };
        let cycle = function("cycle");
        let ordinary = function("ordinary");
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
        let owner = CallableOwner::Function(cycle);
        let mut canonical = CanonicalMirFoundation::empty();
        canonical
            .set_callable_signatures(vec![CallableSignatureRecord::new(
                CallableSignatureSubject::Strong(owner),
                signature.clone(),
            )])
            .unwrap();
        let production = CoreBootstrapBridgeSectionV1::try_new(
            provider,
            EntryMirBridgeBranchV1::Library,
            StrongCallableBridgeSurfaceV1::try_new(vec![StrongCallableBridgeV1::new(
                owner,
                signature.clone(),
            )])
            .unwrap()
            .with_initialization_cycle(cycle)
            .unwrap(),
        )
        .unwrap();
        Self {
            foundation: imported_foundation(provider, canonical),
            production,
            cycle,
            ordinary,
            signature,
        }
    }

    pub(super) fn cycle_record(&self) -> SelectedDependencyMirCallableV1 {
        self.foundation
            .project_initialization_cycle_thrower(
                &self.production,
                self.cycle,
                self.signature.clone(),
            )
            .unwrap()
    }

    pub(super) fn mixed(&self) -> (Module, SelectedExternalMirSet) {
        let function = self.ordinary;
        let declaration = DependencyCallableDeclarationId::Function(function);
        let dependencies = SelectedExternalMirSet::try_from_callables(
            ConeIdentity::SINGLE_FILE,
            vec![
                SelectedDependencyMirCallableV1::try_new(
                    self.foundation.origin(),
                    declaration,
                    StrongCallableDefinitionOwner::Function(function),
                    self.signature.clone(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .with_initialization_cycle(self.cycle_record())
        .unwrap();
        let protocol = dependencies.initialization_cycle().unwrap();
        let selected = dependencies
            .callable_for(self.foundation.origin(), declaration)
            .unwrap();
        let mut module =
            ordinary_module(dependencies.callable_use(selected, GcEffect::NoGc).unwrap());
        let protocol = module.meta.external_callables.alloc(
            dependencies
                .callable_use(protocol, GcEffect::Managed)
                .unwrap(),
        );
        let function = module.functions.iter_mut().next().unwrap().1;
        let statements = &mut function.body.blocks[function.body.entry].statements;
        statements.push(Statement {
            kind: StatementKind::Call(CallEffect::Unit(Call {
                target: CallTarget {
                    kind: CallKind::Direct,
                    callee: Callee::External(protocol),
                },
                args: Vec::new(),
                pending: CoroutinePendingContext::Root,
            })),
            span: SourceSpan::new(0, 0).unwrap(),
        });
        (module, dependencies)
    }
}

pub(super) fn seal(
    module: Module,
    dependencies: &SelectedExternalMirSet,
) -> Result<SingleConeStrongMirInput, SingleConeStrongMirInputError> {
    let foundation = OdrFreeMirFoundation::from_module(&module).unwrap();
    let production = CoreBootstrapBridgeSectionV1::try_new(
        module.cone,
        EntryMirBridgeBranchV1::Library,
        StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation),
    )
    .unwrap();
    SingleConeStrongMirInput::try_new(
        module,
        foundation,
        production,
        Vec::new(),
        StrongExternalCallableInput::Selected(dependencies),
    )
}

#[test]
fn projection_uses_the_role_of_the_requested_strong_record() {
    let fixture = Fixture::new();
    let mut canonical = fixture.foundation.canonical.as_ref().clone();
    let mut signatures = canonical.callable_signatures().to_vec();
    signatures.push(CallableSignatureRecord::new(
        CallableSignatureSubject::Strong(CallableOwner::Function(fixture.ordinary)),
        fixture.signature.clone(),
    ));
    canonical.set_callable_signatures(signatures).unwrap();
    let strong = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(
        &OdrFreeMirFoundation::try_new(canonical.clone()).unwrap(),
    )
    .with_initialization_cycle(fixture.cycle)
    .unwrap();
    let production = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        EntryMirBridgeBranchV1::Library,
        strong,
    )
    .unwrap();
    let imported = imported_foundation(fixture.foundation.origin(), canonical);
    assert_eq!(
        imported.project_initialization_cycle_thrower(
            &production,
            fixture.ordinary,
            fixture.signature.clone()
        ),
        Err(ImportedMirCallableProjectionError::InitializationCycleRoleMismatch(fixture.ordinary))
    );
    assert_eq!(
        imported
            .project_initialization_cycle_thrower(
                &production,
                fixture.cycle,
                fixture.signature.clone()
            )
            .unwrap(),
        fixture.cycle_record()
    );
}

#[test]
fn initialization_projection_requires_the_exact_provider_target_and_signature() {
    let ordinary = scoop_identity::ConeCoordinate::new("tests", "initialization", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    for (provider, other) in [
        (ConeIdentity::CORE, ordinary),
        (ordinary, ConeIdentity::CORE),
    ] {
        let fixture = Fixture::at(provider);
        let other = Fixture::at(other);
        assert_ne!(fixture.cycle, other.cycle);
        assert_eq!(fixture.cycle_record().provider(), provider);
        assert_eq!(
            fixture.foundation.project_initialization_cycle_thrower(
                &fixture.production,
                other.cycle,
                fixture.signature.clone()
            ),
            Err(ImportedMirCallableProjectionError::MissingStrongSignature(
                other.cycle
            )),
        );
        let wrong_signature = ExactCallableSignature::new(
            Effect::Suspend,
            None,
            Vec::new(),
            crate::core_unit_exact_type(),
        );
        assert_eq!(
            fixture.foundation.project_initialization_cycle_thrower(
                &fixture.production,
                fixture.cycle,
                wrong_signature
            ),
            Err(ImportedMirCallableProjectionError::StrongSignatureMismatch(
                fixture.cycle
            )),
        );
        let empty = imported_foundation(provider, CanonicalMirFoundation::empty());
        assert_eq!(
            empty.project_initialization_cycle_thrower(
                &fixture.production,
                fixture.cycle,
                fixture.signature.clone()
            ),
            Err(ImportedMirCallableProjectionError::MissingStrongSignature(
                fixture.cycle
            )),
        );
    }
}
