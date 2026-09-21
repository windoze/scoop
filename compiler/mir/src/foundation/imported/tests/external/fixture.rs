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
        let function = |name| {
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    ConeIdentity::CORE,
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
            ConeIdentity::CORE,
            CoreMirBridgeBranchV1::Core(CoreMirBridgeV1::new(
                CoreMirInitializationCycleThrowerV1::new(cycle, owner).unwrap(),
            )),
            EntryMirBridgeBranchV1::Library,
            StrongCallableBridgeSurfaceV1::try_new(vec![StrongCallableBridgeV1::new(
                owner,
                signature.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
        Self {
            foundation: imported_foundation(canonical),
            production,
            cycle,
            ordinary,
            signature,
        }
    }

    pub(super) fn mixed(
        &self,
        duplicate: bool,
    ) -> (Module, SelectedImportedMirSet<'_>, SelectedDependencyMirSet) {
        let mut protocols = SelectedImportedMirSet::new(&self.foundation, &self.production);
        let protocol = protocols
            .insert(
                self.foundation
                    .project_initialization_cycle_thrower(
                        &self.production,
                        self.cycle,
                        self.signature.clone(),
                    )
                    .unwrap(),
            )
            .unwrap();
        let function = if duplicate { self.cycle } else { self.ordinary };
        let declaration = DependencyCallableDeclarationId::Function(function);
        let dependencies = SelectedDependencyMirSet::try_from_callables(
            ConeIdentity::SINGLE_FILE,
            vec![
                SelectedDependencyMirCallableV1::try_new(
                    ConeIdentity::CORE,
                    declaration,
                    StrongCallableDefinitionOwner::Function(function),
                    self.signature.clone(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let selected = dependencies
            .callable_for(ConeIdentity::CORE, declaration)
            .unwrap();
        let mut module =
            ordinary_module(dependencies.callable_use(selected, GcEffect::NoGc).unwrap());
        let protocol = module
            .meta
            .external_callables
            .alloc(protocols.callable_use(protocol).unwrap());
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
        (module, protocols, dependencies)
    }
}

pub(super) fn seal(
    module: Module,
    protocols: &SelectedImportedMirSet<'_>,
    dependencies: &SelectedDependencyMirSet,
) -> Result<SingleConeStrongMirInput, SingleConeStrongMirInputError> {
    let foundation = OdrFreeMirFoundation::from_module(&module).unwrap();
    let production = CoreBootstrapBridgeSectionV1::try_new(
        module.cone,
        CoreMirBridgeBranchV1::NotCore,
        EntryMirBridgeBranchV1::Library,
        StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation),
    )
    .unwrap();
    SingleConeStrongMirInput::try_new_with_dependencies(
        module,
        foundation,
        production,
        Vec::new(),
        StrongImportedCoreInput::Selected(protocols),
        StrongImportedDependencyInput::Selected(dependencies),
    )
}
