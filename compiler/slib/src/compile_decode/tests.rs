use scoop_hir::{
    CanonicalHirFoundation, NativeBoundaryCLayoutPolicy, NativeBoundaryNominalShape,
    NativeBoundaryTypeDefinitionRecord,
};
use scoop_identity::{
    CanonicalCAbiFunctionSignature, CanonicalCAbiReturn, CanonicalCAbiSignatureFingerprintRecord,
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, CapabilityId, CborIdentityRecord,
    ConeCoordinate, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOrigin,
    DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerChain, ExactCallableSignature,
    ExactTypeKey, GcEffect, LayoutKey, NativeExternalContract, NativeExternalContractFingerprint,
    NativeExternalContractRecord, NativeExternalSymbolKey, NativeLibraryBinding, NonEmptyVec,
    NormalizedSourcePath, PackagePath, PersistentFunctionId, RepresentationRole, ScoopAbiReturn,
    SemanticIdentitySession, SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn,
    SourceCallingConvention, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceExternFunctionAbi, SourceIdentity, SourceNativeExternalContract,
    SourceNativeExternalContractKey, SourceNativeExternalContractRecord,
    SourceNativeLibraryBinding, SourceNativeSymbol, SourceScoopAbiFunctionSignature, SourceSpan,
};
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::CanonicalMirFoundation;

use scoop_wire::encode;

use super::*;
use crate::{
    ConeKind, ConeRecord, ConeSourceForm, DependencyRecord, HirFingerprint,
    IdentityFoundationArtifact, IdentityFoundationArtifactInput, LirFingerprint, ManifestSection,
    MirFingerprint, ProducerRecord, SlibDiagnostic, SlibErrorCode,
};

mod property_tests;

#[test]
fn graph_decodes_identity_checks_and_structurally_validates_all_foundation_layers() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let unit = CoreBuiltinNominal::Unit.identity_record();
    let exact_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit.id())).unwrap();
    let exact = exact_record.id();
    let layout_record = CborIdentityRecord::from_key(LayoutKey::new(
        exact,
        selection.target().wire_id(),
        RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let layout = layout_record.id();
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit, CoreBuiltinNominal::Any.identity_record()])
        .unwrap();
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_exact_types(vec![exact_record]).unwrap();
    let mut lir = CanonicalLirFoundation::empty();
    lir.set_materialized_exact_types(vec![exact]).unwrap();
    lir.set_layouts(vec![layout_record]).unwrap();
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let artifact = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("test").unwrap(),
        cone.clone(),
        selection,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap();
    let envelope = crate::DecodedSlibEnvelope::open(artifact.as_bytes(), selection).unwrap();

    let decoded = envelope
        .validate_graph()
        .unwrap()
        .decode_identity_foundations()
        .unwrap();

    assert_eq!(decoded.identity(), cone.identity());
    assert_eq!(encode(decoded.hir_wire()).unwrap(), encode(&hir).unwrap());
    assert_eq!(encode(decoded.mir_wire()).unwrap(), encode(&mir).unwrap());
    assert_eq!(encode(decoded.lir_wire()).unwrap(), encode(&lir).unwrap());

    let checked = decoded.validate_identities().unwrap();
    assert_eq!(checked.identity(), cone.identity());
    assert_eq!(checked.identity_count(), 5);
    assert_eq!(encode(checked.hir_wire()).unwrap(), encode(&hir).unwrap());
    assert_eq!(encode(checked.mir_wire()).unwrap(), encode(&mir).unwrap());
    assert_eq!(encode(checked.lir_wire()).unwrap(), encode(&lir).unwrap());

    let validated = checked.validate_structure().unwrap();
    assert_eq!(validated.identity(), cone.identity());
    assert_eq!(validated.identity_count(), 5);
    assert_eq!(validated.hir().counts().types, 2);
    assert_eq!(validated.mir().counts().exact_types, 1);
    assert_eq!(validated.lir().counts().materialized_exact_types, 1);
    assert_eq!(validated.lir().counts().layouts, 1);
    assert_eq!(encode(validated.hir()).unwrap(), encode(&hir).unwrap());
    assert_eq!(encode(validated.mir()).unwrap(), encode(&mir).unwrap());
    assert_eq!(encode(validated.lir()).unwrap(), encode(&lir).unwrap());

    let mut session = SemanticIdentitySession::new();
    let compiled = validated
        .validate_native_boundary_source()
        .unwrap()
        .validate_target()
        .unwrap()
        .commit(&mut session)
        .unwrap();
    assert_eq!(compiled.identity(), cone.identity());
    assert_eq!(session.origin_count(), 1);
    assert_eq!(session.entity_count(), 4);
    assert_eq!(compiled.hir().identity_count(), 2);
    assert_eq!(compiled.mir().identity_count(), 1);
    assert_eq!(compiled.lir().identity_count(), 1);
    assert!(compiled.lir().identity(exact).is_none());
    assert_eq!(
        compiled
            .mir()
            .identity(exact)
            .expect("MIR exact identity")
            .persistent(),
        exact
    );
    assert_eq!(
        compiled
            .lir()
            .identity(layout)
            .expect("LIR layout identity")
            .persistent(),
        layout
    );
    assert_eq!(encode(compiled.hir()).unwrap(), encode(&hir).unwrap());
    assert_eq!(encode(compiled.mir()).unwrap(), encode(&mir).unwrap());
    assert_eq!(encode(compiled.lir()).unwrap(), encode(&lir).unwrap());
}

#[test]
fn native_boundary_source_validation_rejects_an_unrelated_witness() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let unit = CoreBuiltinNominal::Unit.identity_record();
    let witness = NativeBoundaryTypeDefinitionRecord::new(
        unit.key(),
        &[0],
        NativeBoundaryNominalShape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
            fields: Vec::new(),
        },
    )
    .unwrap();
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit, CoreBuiltinNominal::Any.identity_record()])
        .unwrap();
    hir.set_native_boundary_types(vec![witness]).unwrap();
    let artifact = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("test").unwrap(),
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        selection,
        &hir,
        &CanonicalMirFoundation::empty(),
        &CanonicalLirFoundation::empty(),
    ))
    .unwrap();

    let error = crate::DecodedSlibEnvelope::open(artifact.as_bytes(), selection)
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_identity_foundations()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_structure()
        .unwrap()
        .validate_native_boundary_source()
        .err()
        .expect("unrelated witness must be rejected");

    assert!(matches!(
        error,
        NativeBoundaryCompileError::UnrelatedDefinition {
            owner: scoop_hir::NativeBoundaryNominalOwner::Concrete(_)
        }
    ));
}

#[test]
fn direct_dependency_native_boundary_requires_the_closure_capability() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let (artifact, _) = scoop_native_function_artifact_with_options(GcEffect::Managed, false, true);

    let graph = crate::DecodedSlibEnvelope::open(artifact.as_bytes(), selection)
        .unwrap()
        .validate_graph()
        .unwrap();
    assert_eq!(
        graph.direct_dependencies()[0].coordinate(),
        &ConeCoordinate::reserved_core()
    );
    let error = graph
        .decode_identity_foundations()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_structure()
        .unwrap()
        .validate_native_boundary_source()
        .err()
        .expect("a missing native-boundary witness must be rejected");

    assert!(matches!(
        error,
        NativeBoundaryCompileError::ClosureRequired {
            owner: scoop_hir::NativeBoundaryNominalOwner::Concrete(_)
        }
    ));
    assert_eq!(
        error.diagnostic().code(),
        SlibErrorCode::CapabilityNativeBoundaryClosureRequired
    );
    assert!(
        error
            .to_string()
            .starts_with("SLIB_CAPABILITY_NATIVE_BOUNDARY_CLOSURE_REQUIRED:")
    );
}

#[test]
fn compile_recomputes_a_target_native_function_contract() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let artifact = native_function_artifact("native_entry");
    let mut session = SemanticIdentitySession::new();

    let compiled = compile(artifact.as_bytes(), selection, &mut session);

    assert_eq!(compiled.lir().counts().native_contracts, 1);
    assert_eq!(compiled.lir().counts().c_abi_signatures, 1);
}

#[test]
fn compile_rejects_a_structurally_valid_but_wrong_target_symbol() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let artifact = native_function_artifact("different_target_symbol");
    let error = crate::DecodedSlibEnvelope::open(artifact.as_bytes(), selection)
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_identity_foundations()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_structure()
        .unwrap()
        .validate_native_boundary_source()
        .unwrap()
        .validate_target()
        .err()
        .expect("target symbol must be derived from the source contract");

    assert!(matches!(
        error,
        NativeBoundaryCompileError::Target(NativeBoundaryTargetError::NativeContractMismatch)
    ));
}

#[test]
fn compile_commit_reuses_world_ids_and_rejects_origin_conflicts_atomically() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let core = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let base = foundation_artifact(core.clone(), None);
    let changed = foundation_artifact(
        core,
        Some(ExactTypeKey::Tuple(NonEmptyVec::from_first(
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(
                CoreBuiltinNominal::Unit.identity_record().id(),
            ))
            .unwrap()
            .id(),
            [],
        ))),
    );
    let mut session = SemanticIdentitySession::new();

    let first = compile(base.as_bytes(), selection, &mut session);
    let first_exact = first.mir().identity_count();
    let origins = session.origin_count();
    let entities = session.entity_count();
    let repeated = compile(base.as_bytes(), selection, &mut session);

    assert_eq!(repeated.mir().identity_count(), first_exact);
    assert_eq!(session.origin_count(), origins);
    assert_eq!(session.entity_count(), entities);

    let result = crate::DecodedSlibEnvelope::open(changed.as_bytes(), selection)
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_identity_foundations()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_structure()
        .unwrap()
        .validate_native_boundary_source()
        .unwrap()
        .validate_target()
        .unwrap()
        .commit(&mut session);
    let error = match result {
        Ok(_) => panic!("changed fingerprints must not commit under an existing origin"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        CompileCommitError::SemanticImport(
            scoop_identity::SemanticIdentityImportError::OriginConflict { .. }
        )
    ));
    assert_eq!(session.origin_count(), origins);
    assert_eq!(session.entity_count(), entities);
}

#[test]
fn imported_ids_are_session_local_but_reencoding_ignores_prior_allocations() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let unit_exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
    .id();
    let earlier = (0..256)
        .map(|arity| {
            ExactTypeKey::Tuple(NonEmptyVec::from_first(unit_exact, vec![unit_exact; arity]))
        })
        .find(|key| CborIdentityRecord::from_key(key.clone()).unwrap().id() < unit_exact)
        .expect("a deterministic exact identity sorts before Unit");
    let core = foundation_artifact(
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        None,
    );
    let prefill = foundation_artifact(
        ConeRecord::new(
            ConeCoordinate::reserved_single_file(),
            ConeKind::Executable,
            ConeSourceForm::SingleFile,
        )
        .unwrap(),
        Some(earlier),
    );

    let mut fresh = SemanticIdentitySession::new();
    let fresh_compile = compile(core.as_bytes(), selection, &mut fresh);
    let fresh_id = fresh_compile
        .mir()
        .identity(unit_exact)
        .expect("fresh Unit exact identity");

    let mut occupied = SemanticIdentitySession::new();
    let _ = compile(prefill.as_bytes(), selection, &mut occupied);
    let occupied_compile = compile(core.as_bytes(), selection, &mut occupied);
    let occupied_id = occupied_compile
        .mir()
        .identity(unit_exact)
        .expect("reused Unit exact identity");

    assert_ne!(fresh_id.session_index(), occupied_id.session_index());
    assert_eq!(fresh_id.persistent(), occupied_id.persistent());
    assert_eq!(
        encode(fresh_compile.mir()).unwrap(),
        encode(occupied_compile.mir()).unwrap()
    );
    assert_eq!(occupied.origin_count(), 2);
    assert_eq!(occupied.entity_count(), 4);
}

#[test]
fn scoop_extern_gc_effect_changes_the_target_contract_fingerprint() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let (managed, managed_fingerprint) = scoop_native_function_artifact(GcEffect::Managed);
    let (no_gc, no_gc_fingerprint) = scoop_native_function_artifact(GcEffect::NoGc);

    assert_ne!(managed_fingerprint, no_gc_fingerprint);
    let mut session = SemanticIdentitySession::new();
    let managed = compile(managed.as_bytes(), selection, &mut session);
    assert_eq!(managed.identity(), ConeIdentity::SINGLE_FILE);

    let mut session = SemanticIdentitySession::new();
    let no_gc = compile(no_gc.as_bytes(), selection, &mut session);
    assert_eq!(no_gc.identity(), ConeIdentity::SINGLE_FILE);
}

fn native_function_artifact(target_symbol: &str) -> IdentityFoundationArtifact {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_single_file(),
        ConeKind::Executable,
        ConeSourceForm::SingleFile,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            cone.identity(),
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("nativeFunction").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
        CborIdentityRecord::from_key(declaration.clone()).unwrap();
    let source_contract = SourceNativeExternalContractRecord::new(
        SourceNativeExternalContractKey::function(&declaration).unwrap(),
        SourceNativeExternalContract::Function {
            symbol: SourceNativeSymbol::new("native_entry").unwrap(),
            library: SourceNativeLibraryBinding::DefaultNativeNamespace,
            abi: SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
                Vec::new(),
                SourceCAbiReturn::Void,
            )),
            calling_convention: SourceCallingConvention::Cdecl,
        },
    )
    .unwrap();
    let source_identity = SourceIdentity::new(
        cone.identity(),
        NormalizedSourcePath::new("main.scoop").unwrap(),
    )
    .unwrap();
    let source_context = CborIdentityRecord::from_key(SourceContextKey::File {
        source: source_identity.clone(),
    })
    .unwrap();
    let span = SourceSpan::new(0, 15).unwrap();
    let origin =
        DefinitionOrigin::new(source_identity.clone(), span, source_context.key()).unwrap();
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_sources(vec![
        scoop_hir::SourceRecord::from_utf8(
            source_identity,
            "extern fun nativeFunction(): Unit",
            [span.start_byte(), span.end_byte()],
        )
        .unwrap(),
    ])
    .unwrap();
    hir.set_types(vec![
        CoreBuiltinNominal::Unit.identity_record(),
        CoreBuiltinNominal::Any.identity_record(),
    ])
    .unwrap();
    hir.set_functions(vec![function.clone()]).unwrap();
    hir.set_source_contexts(vec![source_context]).unwrap();
    hir.set_source_native_contracts(vec![source_contract.clone()])
        .unwrap();
    hir.set_definition_origins(vec![
        DefinitionOriginRecord::new(
            DefinitionOriginSubject::Function(function.id()),
            origin.clone(),
        ),
        DefinitionOriginRecord::new(
            DefinitionOriginSubject::SourceNativeContract(source_contract.id()),
            origin,
        ),
    ])
    .unwrap();

    let signature = CanonicalCAbiSignatureFingerprintRecord::new(
        CanonicalCAbiFunctionSignature::cdecl(Vec::new(), CanonicalCAbiReturn::Void),
    )
    .unwrap();
    let target_contract = NativeExternalContractRecord::new(
        source_contract.id(),
        NativeExternalSymbolKey::darwin_macho_external(
            &SourceNativeSymbol::new(target_symbol).unwrap(),
        )
        .unwrap(),
        NativeExternalContract::c_function(
            NativeLibraryBinding::DefaultNativeNamespace,
            signature.signature().clone(),
        ),
    )
    .unwrap();
    let mut lir = CanonicalLirFoundation::empty();
    lir.set_native_contracts(vec![target_contract]).unwrap();
    lir.set_c_abi_signatures(vec![signature]).unwrap();

    IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("test").unwrap(),
        cone,
        selection,
        &hir,
        &CanonicalMirFoundation::empty(),
        &lir,
    ))
    .unwrap()
}

fn scoop_native_function_artifact(
    gc_effect: GcEffect,
) -> (
    IdentityFoundationArtifact,
    NativeExternalContractFingerprint,
) {
    scoop_native_function_artifact_with_options(gc_effect, true, false)
}

fn scoop_native_function_artifact_with_options(
    gc_effect: GcEffect,
    include_witness: bool,
    include_core_dependency: bool,
) -> (
    IdentityFoundationArtifact,
    NativeExternalContractFingerprint,
) {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_single_file(),
        ConeKind::Executable,
        ConeSourceForm::SingleFile,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            cone.identity(),
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("nativeFunction").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
        CborIdentityRecord::from_key(declaration.clone()).unwrap();
    let unit = CoreBuiltinNominal::Unit.identity_record();
    let unit_signature = SignatureTypeKey::Nominal(unit.id());
    let unit_exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit.id())).unwrap();
    let source_contract = SourceNativeExternalContractRecord::new(
        SourceNativeExternalContractKey::function(&declaration).unwrap(),
        SourceNativeExternalContract::Function {
            symbol: SourceNativeSymbol::new("native_entry").unwrap(),
            library: SourceNativeLibraryBinding::DefaultNativeNamespace,
            abi: SourceExternFunctionAbi::Scoop {
                signature: SourceScoopAbiFunctionSignature::new(Vec::new(), unit_signature),
                gc_effect,
            },
            calling_convention: SourceCallingConvention::Cdecl,
        },
    )
    .unwrap();
    let source_identity = SourceIdentity::new(
        cone.identity(),
        NormalizedSourcePath::new("main.scoop").unwrap(),
    )
    .unwrap();
    let source_context = CborIdentityRecord::from_key(SourceContextKey::File {
        source: source_identity.clone(),
    })
    .unwrap();
    let span = SourceSpan::new(0, 15).unwrap();
    let origin =
        DefinitionOrigin::new(source_identity.clone(), span, source_context.key()).unwrap();
    let unit_witness = NativeBoundaryTypeDefinitionRecord::new(
        unit.key(),
        &[0],
        NativeBoundaryNominalShape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
            fields: Vec::new(),
        },
    )
    .unwrap();
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_sources(vec![
        scoop_hir::SourceRecord::from_utf8(
            source_identity,
            "extern fun nativeFunction(): Unit",
            [span.start_byte(), span.end_byte()],
        )
        .unwrap(),
    ])
    .unwrap();
    hir.set_types(vec![unit, CoreBuiltinNominal::Any.identity_record()])
        .unwrap();
    hir.set_functions(vec![function.clone()]).unwrap();
    hir.set_exact_types(vec![unit_exact.clone()]).unwrap();
    hir.set_source_contexts(vec![source_context]).unwrap();
    hir.set_source_native_contracts(vec![source_contract.clone()])
        .unwrap();
    hir.set_native_boundary_types(
        include_witness
            .then_some(unit_witness)
            .into_iter()
            .collect(),
    )
    .unwrap();
    hir.set_definition_origins(vec![
        DefinitionOriginRecord::new(
            DefinitionOriginSubject::Function(function.id()),
            origin.clone(),
        ),
        DefinitionOriginRecord::new(
            DefinitionOriginSubject::SourceNativeContract(source_contract.id()),
            origin,
        ),
    ])
    .unwrap();

    let signature = CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            None,
            Vec::new(),
            unit_exact.id(),
        ),
        Vec::new(),
        ScoopAbiReturn::UnitVoid,
        gc_effect,
    )
    .unwrap();
    let target_contract = NativeExternalContractRecord::new(
        source_contract.id(),
        NativeExternalSymbolKey::darwin_macho_external(
            &SourceNativeSymbol::new("native_entry").unwrap(),
        )
        .unwrap(),
        NativeExternalContract::scoop_function(
            NativeLibraryBinding::DefaultNativeNamespace,
            signature,
            scoop_identity::TargetCallingConvention::Cdecl,
        ),
    )
    .unwrap();
    let fingerprint = target_contract.fingerprint();
    let mut lir = CanonicalLirFoundation::empty();
    lir.set_native_contracts(vec![target_contract]).unwrap();
    let dependencies = include_core_dependency
        .then(|| {
            DependencyRecord::new(
                ConeCoordinate::reserved_core(),
                HirFingerprint::from_array([1; 32]),
                MirFingerprint::from_array([2; 32]),
                LirFingerprint::from_array([3; 32]),
            )
            .unwrap()
        })
        .into_iter()
        .collect();
    let mir = CanonicalMirFoundation::empty();
    let input = IdentityFoundationArtifactInput::new(
        ProducerRecord::new("test").unwrap(),
        cone,
        selection,
        &hir,
        &mir,
        &lir,
    )
    .with_direct_dependencies(dependencies);
    let artifact = IdentityFoundationArtifact::write(input).unwrap();
    (artifact, fingerprint)
}

fn foundation_artifact(
    cone: ConeRecord,
    extra_exact: Option<ExactTypeKey>,
) -> IdentityFoundationArtifact {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let unit = CoreBuiltinNominal::Unit.identity_record();
    let unit_exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit.id())).unwrap();
    let mut exact_types = vec![unit_exact];
    if let Some(key) = extra_exact {
        exact_types.push(CborIdentityRecord::from_key(key).unwrap());
    }
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit, CoreBuiltinNominal::Any.identity_record()])
        .unwrap();
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_exact_types(exact_types).unwrap();
    let lir = CanonicalLirFoundation::empty();
    IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("test").unwrap(),
        cone,
        selection,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap()
}

fn compile<'input>(
    bytes: &'input [u8],
    selection: ValidatedLirTargetSelection,
    session: &mut SemanticIdentitySession,
) -> ValidatedCompileArtifact<'input, IdentityFoundationProfile> {
    crate::DecodedSlibEnvelope::open(bytes, selection)
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_identity_foundations()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_structure()
        .unwrap()
        .validate_native_boundary_source()
        .unwrap()
        .validate_target()
        .unwrap()
        .commit(session)
        .unwrap()
}

#[test]
fn structural_validation_reports_the_failing_layer() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let artifact = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("test").unwrap(),
        cone,
        selection,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap();
    let checked = crate::DecodedSlibEnvelope::open(artifact.as_bytes(), selection)
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_identity_foundations()
        .unwrap()
        .validate_identities()
        .unwrap();

    assert!(matches!(
        checked.validate_structure(),
        Err(FoundationStructureValidationError::Hir(_))
    ));
}

#[test]
fn unknown_compile_required_manifest_capability_stops_foundation_decode() {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let capability = CapabilityId::new("org.scoop-lang.test", "compile", 1).unwrap();
    let metadata = crate::IdentityFoundationMetadata::new(&hir, &mir, &lir).unwrap();
    let compatibility = crate::CompatibilityRecord::new(
        selection,
        crate::ArtifactCapabilityProfile::IDENTITY_FOUNDATION,
    )
    .unwrap();
    let fingerprints = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility,
        &[],
        std::slice::from_ref(metadata.hir_section()),
        std::slice::from_ref(metadata.mir_section()),
        std::slice::from_ref(metadata.lir_section()),
    )
    .unwrap();
    let members = metadata.into_members(cone.identity()).unwrap();
    let manifest = crate::BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone,
        Vec::new(),
        &members,
        fingerprints,
        vec![
            ManifestSection::new(capability.clone(), MemberPurposeSet::COMPILE, Vec::new())
                .unwrap(),
        ],
    )
    .unwrap();
    let archive = crate::CanonicalSlibArchive::write_bootstrap(&manifest, members).unwrap();
    let graph = crate::DecodedSlibEnvelope::open(archive.as_bytes(), selection)
        .unwrap()
        .validate_graph()
        .unwrap();

    assert!(matches!(
        graph.decode_identity_foundations(),
        Err(IdentityFoundationDecodeError::UnknownCompileCapability {
            location: None,
            index: 0,
            capability: actual,
        }) if actual == capability
    ));
}
