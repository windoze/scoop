use super::*;

pub(super) struct Fixture {
    provider: ConeIdentity,
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
        Self {
            provider,
            cycle,
            ordinary,
            signature,
        }
    }

    pub(super) fn cycle_record(&self) -> SelectedDependencyMirCallableV1 {
        SelectedDependencyMirCallableV1::try_new(
            self.provider,
            DependencyCallableDeclarationId::Function(self.cycle),
            StrongCallableDefinitionOwner::Function(self.cycle),
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
                    self.provider,
                    declaration,
                    StrongCallableDefinitionOwner::Function(function),
                    self.signature.clone(),
                )
                .unwrap(),
                self.cycle_record(),
            ],
        )
        .unwrap();
        let protocol = dependencies
            .callable_for(
                self.provider,
                StrongCallableDefinitionOwner::Function(self.cycle),
            )
            .unwrap();
        let selected = dependencies
            .callable_for(self.provider, declaration.implementation())
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
