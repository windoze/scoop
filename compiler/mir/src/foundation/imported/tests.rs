use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, Effect, ExactTypeKey, PackagePath,
    PendingIdentityValidation, PersistentExactTypeId, SemanticIdentitySession,
    SemanticOriginFingerprint, SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{
    BasicBlock, Body, Call, CallEffect, CallKind, CallTarget, CallableSignatureRecord,
    CallableSignatureSubject, Callee, CoreBootstrapBridgeSectionV1, CoreMirBridgeBranchV1,
    CoreMirBridgeV1, CoreMirInitializationCycleThrowerV1, CoreShapeSupportSourceInput,
    CoroutinePendingContext, EntryMirBridgeBranchV1, Function, GcEffect, MirMeta, MirOutput,
    Module, OdrFreeMirFoundation, OrdinaryMirOutput, OrdinaryMirOutputError,
    SingleConeStrongMirInput, SourceSpan, Statement, StatementKind, StrongCallableBridgeSurfaceV1,
    StrongCallableBridgeV1, Terminator, Type,
};

#[test]
fn selected_imported_mir_worlds_have_distinct_process_local_brands() {
    assert_ne!(
        next_imported_core_mir_selection(),
        next_imported_core_mir_selection()
    );
}

#[test]
fn selected_callable_derives_the_only_strong_implementation() {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("run").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let definition = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
    let implementation = CallableOwner::Function(definition);
    let cycle_declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("__scoopThrowInitializationCycle").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let cycle_definition =
        PersistentFunctionId::from_source_declaration(&cycle_declaration).unwrap();
    let cycle_implementation = CallableOwner::Function(cycle_definition);
    let mut canonical = CanonicalMirFoundation::empty();
    canonical
        .set_callable_signatures(vec![
            CallableSignatureRecord::new(
                CallableSignatureSubject::Strong(implementation),
                signature.clone(),
            ),
            CallableSignatureRecord::new(
                CallableSignatureSubject::Strong(cycle_implementation),
                signature.clone(),
            ),
        ])
        .unwrap();
    let foundation = imported_foundation(canonical.clone());
    let other_foundation = imported_foundation(canonical);
    let production = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        CoreMirBridgeBranchV1::Core(
            CoreMirBridgeV1::try_new(
                Vec::new(),
                CoreMirInitializationCycleThrowerV1::new(cycle_definition, cycle_implementation)
                    .unwrap(),
            )
            .unwrap(),
        ),
        EntryMirBridgeBranchV1::Library,
        StrongCallableBridgeSurfaceV1::try_new(vec![
            StrongCallableBridgeV1::new(implementation, signature.clone()),
            StrongCallableBridgeV1::new(cycle_implementation, signature.clone()),
        ])
        .unwrap(),
    )
    .unwrap();

    let selected = foundation
        .project_initialization_cycle_thrower(&production, cycle_definition, signature.clone())
        .unwrap();
    let foreign = other_foundation
        .project_initialization_cycle_thrower(&production, cycle_definition, signature.clone())
        .unwrap();

    assert!(selected.belongs_to(&foundation, &production));
    assert!(!selected.belongs_to(&other_foundation, &production));
    assert_eq!(
        selected.kind(),
        CoreImportedCallableKind::InitializationCycleThrower
    );
    assert_eq!(selected.definition(), cycle_definition);
    assert_eq!(
        selected.implementation(),
        StrongCallableDefinitionOwner::Function(cycle_definition)
    );
    assert_eq!(selected.signature(), &signature);

    let mut selections = SelectedImportedMirSet::new(&foundation, &production);
    let first = selections.insert(selected.clone()).unwrap();
    assert_eq!(selections.insert(selected).unwrap(), first);
    assert_eq!(selections.len(), 1);
    assert_eq!(
        selections.callable_for_kind(CoreImportedCallableKind::InitializationCycleThrower),
        Some(first)
    );
    assert_eq!(
        selections.callable(first).unwrap().definition(),
        cycle_definition
    );
    assert_eq!(
        selections.insert(foreign),
        Err(ImportedMirSelectionError::ForeignSelection(
            CoreImportedCallableKind::InitializationCycleThrower
        ))
    );
    assert_eq!(selections.len(), 1);

    let callable_use = selections.callable_use(first).unwrap();
    let mut foreign_selections = SelectedImportedMirSet::new(&foundation, &production);
    let foreign_id = foreign_selections
        .insert(
            foundation
                .project_initialization_cycle_thrower(&production, cycle_definition, signature)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(foreign_id, first);
    assert!(matches!(
        OrdinaryMirOutput::try_new(
            ordinary_module(callable_use),
            foreign_selections,
            crate::SelectedDependencyMirSet::empty(ConeIdentity::SINGLE_FILE),
        ),
        Err(OrdinaryMirOutputError::ForeignImportedCallable { index: 0 })
    ));

    let ordinary = OrdinaryMirOutput::try_new(
        ordinary_module(callable_use),
        selections,
        crate::SelectedDependencyMirSet::empty(ConeIdentity::SINGLE_FILE),
    )
    .expect("the ordinary MIR graph and selected sidecar share one brand");
    let (module, selections, dependency_selections) = ordinary.into_parts();
    assert!(dependency_selections.is_empty());
    let retained = module.meta.imported_core_callables.iter().next().unwrap().1;
    assert!(selections.resolve_callable(retained.reference()).is_some());

    let strong_foundation = OdrFreeMirFoundation::from_module(&module).unwrap();
    let ordinary_production = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::SINGLE_FILE,
        CoreMirBridgeBranchV1::NotCore,
        EntryMirBridgeBranchV1::Library,
        StrongCallableBridgeSurfaceV1::try_new(Vec::new()).unwrap(),
    )
    .unwrap();
    let missing_authority_module = ordinary_module(callable_use);
    let missing_authority_foundation =
        OdrFreeMirFoundation::from_module(&missing_authority_module).unwrap();
    let missing_authority_production = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::SINGLE_FILE,
        CoreMirBridgeBranchV1::NotCore,
        EntryMirBridgeBranchV1::Library,
        StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&missing_authority_foundation),
    )
    .unwrap();
    assert!(matches!(
        SingleConeStrongMirInput::try_new(
            missing_authority_module,
            missing_authority_foundation,
            missing_authority_production,
            CoreShapeSupportSourceInput::NotCore,
            crate::StrongImportedCoreInput::Unused,
        ),
        Err(crate::SingleConeStrongMirInputError::MissingImportedCoreAuthority)
    ));
    let strong = SingleConeStrongMirInput::try_new(
        module,
        strong_foundation,
        ordinary_production,
        CoreShapeSupportSourceInput::NotCore,
        crate::StrongImportedCoreInput::Selected(&selections),
    )
    .expect("the strong sealer resolves the exact imported MIR selected set");
    let roots = strong.materialization().imported_core_callable_roots();
    assert_eq!(roots.len(), 1);
    assert_eq!(
        roots[0].kind(),
        CoreImportedCallableKind::InitializationCycleThrower
    );
    assert_eq!(
        roots[0].implementation(),
        StrongCallableDefinitionOwner::Function(cycle_definition)
    );
}

fn ordinary_module(callable: crate::ImportedCoreCallableUse) -> Module {
    let mut imported_core_callables = Arena::new();
    let callable = imported_core_callables.alloc(callable);
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        statements: vec![Statement {
            kind: StatementKind::Call(CallEffect::Unit(Call {
                target: CallTarget {
                    kind: CallKind::Direct,
                    callee: Callee::CoreExternal(callable),
                },
                args: Vec::new(),
                pending: CoroutinePendingContext::Root,
            })),
            span: SourceSpan::new(0, 0).unwrap(),
        }],
        terminator: Terminator::Return { value: None },
        unwind: None,
    });
    let mut functions = Arena::new();
    functions.alloc(Function {
        gc_effect: GcEffect::Managed,
        name: "ordinary".to_string(),
        params: Vec::new(),
        return_ty: Type::Unit,
        body: Body {
            locals: Arena::new(),
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    });
    Module {
        cone: ConeIdentity::SINGLE_FILE,
        functions,
        extern_functions: Arena::new(),
        globals: Arena::new(),
        initialization_units: Arena::new(),
        initialization_failure_roots: Arena::new(),
        objects: Arena::new(),
        object_types: Arena::new(),
        singleton_values: Arena::new(),
        singleton_published_roots: Arena::new(),
        callback_bridges: Arena::new(),
        foreign_callback_adapters: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        function_types: Arena::new(),
        closure_classes: Arena::new(),
        closure_invoke_functions: Arena::new(),
        top_level: Vec::new(),
        strings: Arena::new(),
        structs: Arena::new(),
        enums: Arena::new(),
        classes: Arena::new(),
        interfaces: Arena::new(),
        option_core: Vec::new(),
        output: MirOutput::Library,
        meta: MirMeta {
            imported_core_callables,
            ..MirMeta::default()
        },
    }
}

fn imported_foundation(canonical: CanonicalMirFoundation) -> ImportedMirFoundation {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (_, imported, _) = session
        .import(
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap()
        .into_parts();
    ImportedMirFoundation {
        canonical,
        identities: imported,
    }
}
