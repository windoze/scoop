use super::*;

pub(super) fn other_function_id() -> PersistentFunctionId {
    CborIdentityRecord::from_key(source_function("absent"))
        .unwrap()
        .id()
}

pub(super) fn source_function(name: &str) -> SourceDeclarationKey {
    source_function_in(ConeIdentity::CORE, name)
}

pub(super) fn source_function_in(cone: ConeIdentity, name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    )
}

pub(super) fn entry_source(
    declaration: &CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    exact_unit: PersistentExactTypeId,
) -> ExecutableSourceEntryIdentity {
    ExecutableSourceEntryIdentity::try_new(
        declaration,
        ExactOrdinaryNoArgUnitSignature::new(exact_unit),
    )
    .unwrap()
}

pub(super) fn decode(
    section: &CoreBootstrapBridgeSectionV1,
) -> DecodedCoreBootstrapBridgeSectionV1 {
    decode_canonical(&encode(section).unwrap(), DecodeLimits::default()).unwrap()
}

pub(super) struct Fixture {
    pub(super) hir: CanonicalHirFoundation,
    pub(super) mir: CanonicalMirFoundation,
    pub(super) section: CoreBootstrapBridgeSectionV1,
    pub(super) function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    pub(super) other_function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    pub(super) exact_unit: PersistentExactTypeId,
}

pub(super) fn fixture() -> Fixture {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("printLine").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = CborIdentityRecord::from_key(declaration.clone()).unwrap();
    let other_function = CborIdentityRecord::from_key(source_function("other")).unwrap();
    let binding = CborIdentityRecord::from_key(ExportBindingKey::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        CanonicalIdentifier::new("printLine").unwrap(),
        BindingTarget::function(&declaration).unwrap(),
    ))
    .unwrap();
    let unit_record = CoreBuiltinNominal::Unit.identity_record();
    let unit_type = unit_record.id();
    let exact_unit_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit_type)).unwrap();
    let exact_unit = exact_unit_record.id();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact_unit);

    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit_record]).unwrap();
    hir.set_functions(vec![function.clone(), other_function.clone()])
        .unwrap();
    hir.set_exact_types(vec![exact_unit_record]).unwrap();
    hir.set_export_bindings(vec![binding.clone()]).unwrap();

    let mut mir = CanonicalMirFoundation::empty();
    mir.set_callable_signatures(vec![
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(function.id())),
            signature.clone(),
        ),
        CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(CallableOwner::Function(other_function.id())),
            signature.clone(),
        ),
    ])
    .unwrap();
    let strong_callable_bridges = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(
        &OdrFreeMirFoundation::try_new(mir.clone()).unwrap(),
    );
    let section = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        EntryMirBridgeBranchV1::Library,
        strong_callable_bridges
            .with_initialization_cycle(other_function.id())
            .unwrap(),
    )
    .unwrap();

    Fixture {
        hir,
        mir,
        section,
        function,
        other_function,
        exact_unit,
    }
}

pub(super) fn validate_foundations(
    fixture: &Fixture,
) -> (ValidatedIdentityGraph, ValidatedMirFoundation) {
    validate_mir(&fixture.hir, &fixture.mir)
}

pub(super) fn generated_callable_sorting_before(
    function: PersistentFunctionId,
) -> CborIdentityRecord<scoop_identity::PersistentGeneratedCallableId, GeneratedCallableKey> {
    for ordinal in 0..1_000 {
        let record = CborIdentityRecord::from_key(GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(function),
            role: LexicalCallableRole::LambdaBody,
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, ordinal),
                [],
            ),
        })
        .unwrap();
        if record.id().as_array() < function.as_array() {
            return record;
        }
    }
    panic!("expected a generated callable sorting before the source function")
}

pub(super) fn validate_mir(
    hir: &CanonicalHirFoundation,
    mir: &CanonicalMirFoundation,
) -> (ValidatedIdentityGraph, ValidatedMirFoundation) {
    let hir: scoop_hir::DecodedHirFoundation =
        decode_canonical(&encode(hir).unwrap(), DecodeLimits::default()).unwrap();
    let mir: DecodedMirFoundation =
        decode_canonical(&encode(mir).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    hir.register_identities(&mut pending).unwrap();
    mir.register_identities(&mut pending).unwrap();
    hir.resolve_identities(&mut pending).unwrap();
    mir.resolve_identities(&mut pending).unwrap();
    let mut identities = pending.finish().unwrap();
    let foundation = mir
        .validate(
            &mut identities,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
        .unwrap();
    (identities, foundation)
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
