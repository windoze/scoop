use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_identity::{
    CallableOwner, ConeIdentity, ExactCallableSignature, ImportedIdentityId, ImportedIdentityMap,
    MirIdentityLayer, PersistentExportBindingId, PersistentFunctionId, PersistentId,
    StrongCallableDefinitionOwner,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalMirFoundation, MirFoundationCounts, OdrFreeMirFoundation, ValidatedMirFoundation,
};

/// Session-local MIR identity. It cannot be interchanged with imported HIR or
/// LIR identities even when the persistent kind is the same.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedMirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedMirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// The MIR identity foundation after atomic import into a semantic session.
pub struct ImportedMirFoundation {
    canonical: CanonicalMirFoundation,
    identities: ImportedIdentityMap<MirIdentityLayer>,
}

impl ImportedMirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedMirFoundation,
        identities: ImportedIdentityMap<MirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeMirFoundation,
        identities: ImportedIdentityMap<MirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> MirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedMirId<I>> {
        self.identities.get(id).map(ImportedMirId)
    }

    /// Projects a callable only after replaying the complete core MIR bridge
    /// relation against this exact imported foundation.
    pub fn project_core_callable<'a>(
        &'a self,
        production: &'a crate::CoreBootstrapBridgeSectionV1,
        binding: PersistentExportBindingId,
        definition: PersistentFunctionId,
        signature: ExactCallableSignature,
    ) -> Result<SelectedImportedMirCallable<'a>, ImportedMirCallableProjectionError> {
        if self.origin() != ConeIdentity::CORE {
            return Err(ImportedMirCallableProjectionError::FoundationNotCore(
                self.origin(),
            ));
        }
        let crate::CoreMirBridgeBranchV1::Core(core_bridge) = production.core_bridge() else {
            return Err(ImportedMirCallableProjectionError::MissingCoreBridge);
        };
        let bridge = core_bridge
            .callable_targets()
            .iter()
            .find(|bridge| bridge.binding() == binding)
            .ok_or(ImportedMirCallableProjectionError::MissingCallable(binding))?;
        let implementation = CallableOwner::Function(definition);
        if bridge.definition() != definition || bridge.implementation() != implementation {
            return Err(ImportedMirCallableProjectionError::CallableMismatch(
                binding,
            ));
        }
        let strong_bridge = production
            .strong_callable_bridges()
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == implementation)
            .ok_or(ImportedMirCallableProjectionError::MissingStrongSignature(
                definition,
            ))?;
        if strong_bridge.signature() != &signature {
            return Err(ImportedMirCallableProjectionError::StrongSignatureMismatch(
                definition,
            ));
        }
        let foundation_signature = self
            .canonical
            .callable_signatures()
            .iter()
            .find(|candidate| {
                candidate.subject() == crate::CallableSignatureSubject::Strong(implementation)
            })
            .ok_or(ImportedMirCallableProjectionError::MissingStrongSignature(
                definition,
            ))?;
        if foundation_signature.signature() != &signature {
            return Err(ImportedMirCallableProjectionError::StrongSignatureMismatch(
                definition,
            ));
        }
        Ok(SelectedImportedMirCallable {
            foundation: self,
            production,
            binding,
            definition,
            implementation: StrongCallableDefinitionOwner::Function(definition),
            signature,
        })
    }
}

impl WireEncode for ImportedMirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

/// A param-free callable selected from the trusted core HIR surface and
/// proven against that artifact's MIR bridge.
///
/// Private borrows retain the exact imported foundation and production
/// surface that minted the value. The next stage can therefore reject a
/// selection from another artifact even when all persistent ids and payloads
/// are byte-identical.
#[derive(Clone)]
pub struct SelectedImportedMirCallable<'a> {
    foundation: &'a ImportedMirFoundation,
    production: &'a crate::CoreBootstrapBridgeSectionV1,
    binding: PersistentExportBindingId,
    definition: PersistentFunctionId,
    implementation: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
}

/// Request-local MIR id for one callable selected from trusted core.
/// It cannot be interchanged with its HIR or LIR counterpart.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportedCoreMirCallableId(u32);

/// One MIR callable reference branded by the selected-set world that proved
/// its core implementation bridge.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedCoreMirCallableRef {
    selection: ImportedCoreMirSelectionId,
    callable: ImportedCoreMirCallableId,
}

impl ImportedCoreMirCallableRef {
    pub const fn callable(self) -> ImportedCoreMirCallableId {
        self.callable
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ImportedCoreMirSelectionId(u64);

fn next_imported_core_mir_selection() -> ImportedCoreMirSelectionId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let selection = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("the imported core MIR selection id space is exhausted");
    ImportedCoreMirSelectionId(selection)
}

/// The complete imported callable set for one ordinary MIR lowering.
///
/// Construction is bound to one imported foundation and production section;
/// every inserted selection must retain those exact proofs. The set cannot
/// be assembled from bare persistent identities or signatures.
pub struct SelectedImportedMirSet<'a> {
    foundation: &'a ImportedMirFoundation,
    production: &'a crate::CoreBootstrapBridgeSectionV1,
    selection: ImportedCoreMirSelectionId,
    by_binding: BTreeMap<PersistentExportBindingId, ImportedCoreMirCallableId>,
    callables: Vec<SelectedImportedMirCallable<'a>>,
}

impl<'a> SelectedImportedMirSet<'a> {
    #[doc(hidden)]
    pub fn new(
        foundation: &'a ImportedMirFoundation,
        production: &'a crate::CoreBootstrapBridgeSectionV1,
    ) -> Self {
        Self {
            foundation,
            production,
            selection: next_imported_core_mir_selection(),
            by_binding: BTreeMap::new(),
            callables: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn insert(
        &mut self,
        selected: SelectedImportedMirCallable<'a>,
    ) -> Result<ImportedCoreMirCallableId, ImportedMirSelectionError> {
        if !selected.belongs_to(self.foundation, self.production) {
            return Err(ImportedMirSelectionError::ForeignSelection(
                selected.binding(),
            ));
        }
        if let Some(&id) = self.by_binding.get(&selected.binding()) {
            let retained = &self.callables[id.0 as usize];
            if retained.definition() != selected.definition()
                || retained.implementation() != selected.implementation()
                || retained.signature() != selected.signature()
            {
                return Err(ImportedMirSelectionError::ConflictingSelection(
                    selected.binding(),
                ));
            }
            return Ok(id);
        }
        let id = ImportedCoreMirCallableId(
            u32::try_from(self.callables.len())
                .expect("one MIR request cannot select more than u32::MAX core callables"),
        );
        self.by_binding.insert(selected.binding(), id);
        self.callables.push(selected);
        Ok(id)
    }

    pub fn callable(
        &self,
        id: ImportedCoreMirCallableId,
    ) -> Option<&SelectedImportedMirCallable<'a>> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_ref(
        &self,
        id: ImportedCoreMirCallableId,
    ) -> Option<ImportedCoreMirCallableRef> {
        self.callable(id).map(|_| ImportedCoreMirCallableRef {
            selection: self.selection,
            callable: id,
        })
    }

    pub fn callable_use(
        &self,
        id: ImportedCoreMirCallableId,
    ) -> Option<crate::ImportedCoreCallableUse> {
        self.callable_ref(id)
            .map(crate::ImportedCoreCallableUse::new)
    }

    pub fn resolve_callable(
        &self,
        reference: ImportedCoreMirCallableRef,
    ) -> Option<&SelectedImportedMirCallable<'a>> {
        (reference.selection == self.selection)
            .then(|| self.callable(reference.callable))
            .flatten()
    }

    pub fn callable_for_binding(
        &self,
        binding: PersistentExportBindingId,
    ) -> Option<ImportedCoreMirCallableId> {
        self.by_binding.get(&binding).copied()
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedMirFoundation,
        production: &crate::CoreBootstrapBridgeSectionV1,
    ) -> bool {
        std::ptr::eq(self.foundation, foundation) && std::ptr::eq(self.production, production)
    }

    #[doc(hidden)]
    pub fn callable_selections(&self) -> impl Iterator<Item = &SelectedImportedMirCallable<'a>> {
        self.callables.iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedMirSelectionError {
    ForeignSelection(PersistentExportBindingId),
    ConflictingSelection(PersistentExportBindingId),
}

impl fmt::Display for ImportedMirSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignSelection(binding) => write!(
                formatter,
                "imported MIR binding {binding} belongs to another core artifact projection"
            ),
            Self::ConflictingSelection(binding) => write!(
                formatter,
                "imported MIR binding {binding} has conflicting selected definitions"
            ),
        }
    }
}

impl std::error::Error for ImportedMirSelectionError {}

impl SelectedImportedMirCallable<'_> {
    pub const fn binding(&self) -> PersistentExportBindingId {
        self.binding
    }

    pub const fn definition(&self) -> PersistentFunctionId {
        self.definition
    }

    pub const fn implementation(&self) -> StrongCallableDefinitionOwner {
        self.implementation
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedMirFoundation,
        production: &crate::CoreBootstrapBridgeSectionV1,
    ) -> bool {
        std::ptr::eq(self.foundation, foundation) && std::ptr::eq(self.production, production)
    }
}

impl fmt::Debug for SelectedImportedMirCallable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedImportedMirCallable")
            .field("binding", &self.binding)
            .field("definition", &self.definition)
            .field("implementation", &self.implementation)
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedMirCallableProjectionError {
    FoundationNotCore(ConeIdentity),
    MissingCoreBridge,
    MissingCallable(PersistentExportBindingId),
    CallableMismatch(PersistentExportBindingId),
    MissingStrongSignature(PersistentFunctionId),
    StrongSignatureMismatch(PersistentFunctionId),
}

impl fmt::Display for ImportedMirCallableProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot project imported MIR callable: {self:?}")
    }
}

impl std::error::Error for ImportedMirCallableProjectionError {}

#[cfg(test)]
mod tests {
    use la_arena::Arena;
    use scoop_identity::{
        BindingTarget, CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, Effect,
        ExactTypeKey, ExportBindingKey, PackagePath, PendingIdentityValidation,
        PersistentExactTypeId, SemanticIdentitySession, SemanticOriginFingerprint,
        SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;
    use crate::{
        BasicBlock, Body, Call, CallEffect, CallKind, CallTarget, CallableSignatureRecord,
        CallableSignatureSubject, Callee, CoreBootstrapBridgeSectionV1, CoreMirBridgeBranchV1,
        CoreMirBridgeV1, CoreMirCallableBridgeV1, CoreShapeSupportSourceInput,
        CoroutinePendingContext, EntryMirBridgeBranchV1, Function, GcEffect, MirMeta, MirOutput,
        Module, OdrFreeMirFoundation, OrdinaryMirOutput, OrdinaryMirOutputError,
        SingleConeStrongMirInput, SingleConeStrongMirInputError, SourceSpan, Statement,
        StatementKind, StrongCallableBridgeSurfaceV1, StrongCallableBridgeV1, Terminator, Type,
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
        let binding = PersistentExportBindingId::from_key(&ExportBindingKey::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            CanonicalIdentifier::new("run").unwrap(),
            BindingTarget::function(&declaration).unwrap(),
        ))
        .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
        let implementation = CallableOwner::Function(definition);
        let mut canonical = CanonicalMirFoundation::empty();
        canonical
            .set_callable_signatures(vec![CallableSignatureRecord::new(
                CallableSignatureSubject::Strong(implementation),
                signature.clone(),
            )])
            .unwrap();
        let foundation = imported_foundation(canonical.clone());
        let other_foundation = imported_foundation(canonical);
        let production = CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::CORE,
            CoreMirBridgeBranchV1::Core(
                CoreMirBridgeV1::try_new(
                    vec![
                        CoreMirCallableBridgeV1::new(binding, definition, implementation).unwrap(),
                    ],
                    Vec::new(),
                )
                .unwrap(),
            ),
            EntryMirBridgeBranchV1::Library,
            StrongCallableBridgeSurfaceV1::try_new(vec![StrongCallableBridgeV1::new(
                implementation,
                signature.clone(),
            )])
            .unwrap(),
        )
        .unwrap();

        let selected = foundation
            .project_core_callable(&production, binding, definition, signature.clone())
            .unwrap();
        let foreign = other_foundation
            .project_core_callable(&production, binding, definition, signature.clone())
            .unwrap();

        assert!(selected.belongs_to(&foundation, &production));
        assert!(!selected.belongs_to(&other_foundation, &production));
        assert_eq!(selected.binding(), binding);
        assert_eq!(selected.definition(), definition);
        assert_eq!(
            selected.implementation(),
            StrongCallableDefinitionOwner::Function(definition)
        );
        assert_eq!(selected.signature(), &signature);

        let mut selections = SelectedImportedMirSet::new(&foundation, &production);
        let first = selections.insert(selected.clone()).unwrap();
        assert_eq!(selections.insert(selected).unwrap(), first);
        assert_eq!(selections.len(), 1);
        assert_eq!(selections.callable_for_binding(binding), Some(first));
        assert_eq!(selections.callable(first).unwrap().definition(), definition);
        assert_eq!(
            selections.insert(foreign),
            Err(ImportedMirSelectionError::ForeignSelection(binding))
        );
        assert_eq!(selections.len(), 1);

        let callable_use = selections.callable_use(first).unwrap();
        let mut foreign_selections = SelectedImportedMirSet::new(&foundation, &production);
        let foreign_id = foreign_selections
            .insert(
                foundation
                    .project_core_callable(&production, binding, definition, signature)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(foreign_id, first);
        assert!(matches!(
            OrdinaryMirOutput::try_new(ordinary_module(callable_use), foreign_selections),
            Err(OrdinaryMirOutputError::ForeignImportedCallable { index: 0 })
        ));

        let ordinary = OrdinaryMirOutput::try_new(ordinary_module(callable_use), selections)
            .expect("the ordinary MIR graph and selected sidecar share one brand");
        let (module, selections) = ordinary.into_parts();
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
        assert!(matches!(
            SingleConeStrongMirInput::try_new(
                module,
                strong_foundation,
                ordinary_production,
                CoreShapeSupportSourceInput::NotCore,
            ),
            Err(SingleConeStrongMirInputError::ImportedCoreCallablesRequireOrdinaryInput)
        ));
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
}
