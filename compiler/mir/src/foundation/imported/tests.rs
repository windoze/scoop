use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, Effect, ExactTypeKey, PackagePath,
    PendingIdentityValidation, PersistentExactTypeId, SemanticIdentitySession,
    SemanticOriginFingerprint, SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{
    BasicBlock, Body, Call, CallEffect, CallKind, CallTarget, CallableSignatureRecord,
    CallableSignatureSubject, Callee, CoreBootstrapBridgeSectionV1, CoroutinePendingContext,
    DependencyMirOutput, DependencyMirOutputError, EntryMirBridgeBranchV1, Function, GcEffect,
    MirMeta, MirOutput, Module, OdrFreeMirFoundation, SingleConeStrongMirInput, SourceSpan,
    Statement, StatementKind, StrongCallableBridgeSurfaceV1, StrongCallableBridgeV1, Terminator,
    Type,
};

mod external;

fn ordinary_module(callable: crate::ExternalCallableUse) -> Module {
    let mut external_callables = Arena::new();
    let callable = external_callables.alloc(callable);
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        statements: vec![Statement {
            kind: StatementKind::Call(CallEffect::Unit(Call {
                target: CallTarget {
                    kind: CallKind::Direct,
                    callee: Callee::External(callable),
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
            external_callables,
            ..MirMeta::default()
        },
    }
}

fn imported_foundation(
    provider: ConeIdentity,
    canonical: CanonicalMirFoundation,
) -> ImportedMirFoundation {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (_, imported, _) = session
        .import(
            provider,
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
