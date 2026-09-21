use std::fmt;
use std::marker::PhantomData;

use scoop_identity::{
    BindingNamespace, ConeIdentity, ExportBindingKey, HirIdentityLayer, ImportedIdentityId,
    ImportedIdentityMap, PersistentExportBindingId, PersistentId, PersistentTypeId,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalHirFoundation, HirFoundationCounts, OdrFreeHirFoundation, ValidatedHirFoundation,
};
use crate::{CoreHirInterfaceV1, CoreTypeTargetV1};

/// Session-local HIR identity. Its type is distinct from current HIR ids and
/// from imported MIR/LIR ids.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedHirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedHirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// The complete HIR identity foundation after atomic import into a semantic
/// session. It is artifact metadata, not a semantic lookup capability.
pub struct ImportedHirFoundation {
    canonical: CanonicalHirFoundation,
    identities: ImportedIdentityMap<HirIdentityLayer>,
}

impl ImportedHirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedHirFoundation,
        identities: ImportedIdentityMap<HirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeHirFoundation,
        identities: ImportedIdentityMap<HirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> HirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedHirId<I>> {
        self.identities.get(id).map(ImportedHirId)
    }

    pub(crate) const fn canonical_for_semantic_authority(&self) -> &CanonicalHirFoundation {
        &self.canonical
    }

    /// Returns source metadata already authenticated as part of this
    /// provider's HIR foundation.
    pub fn source_record(
        &self,
        source: &scoop_identity::SourceIdentity,
    ) -> Option<&crate::SourceRecord> {
        self.canonical
            .sources
            .binary_search_by(|record| record.identity().cmp(source))
            .ok()
            .map(|index| &self.canonical.sources[index])
    }

    /// Returns the canonical key for one provider-owned source context.
    pub fn source_context_key(
        &self,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<&scoop_identity::SourceContextKey> {
        self.canonical
            .source_contexts
            .binary_search_by_key(&context, |record| record.id())
            .ok()
            .map(|index| self.canonical.source_contexts[index].key())
    }

    pub(crate) fn semantic_world_export_binding_key(
        &self,
        id: PersistentExportBindingId,
    ) -> Option<&ExportBindingKey> {
        self.canonical.export_binding_key(id)
    }

    pub(super) fn core_source_type_key(
        &self,
        id: PersistentTypeId,
    ) -> Option<&scoop_identity::SourceDeclarationKey> {
        self.canonical.source_type_key(id)
    }

    pub(super) fn core_source_field_count(&self, owner: PersistentTypeId) -> usize {
        self.canonical.source_field_count(owner)
    }

    /// Projects the type bindings needed during ordinary HIR lowering.
    /// Function and value lookup uses the dependency world.
    pub(super) fn import_core_prelude<'a>(
        &'a self,
        interface: &'a CoreHirInterfaceV1,
    ) -> Result<ImportedHirSet<'a, CorePreludeOnly>, CorePreludeImportError> {
        if self.origin() != ConeIdentity::CORE {
            return Err(CorePreludeImportError::FoundationNotCore(self.origin()));
        }

        let mut bindings = Vec::with_capacity(interface.type_targets().targets().len());
        for target in interface.type_targets().targets() {
            let binding = target.binding();
            let key = self
                .canonical
                .export_binding_key(binding)
                .ok_or(CorePreludeImportError::MissingBindingKey(binding))?;
            if key.exporter() != ConeIdentity::CORE {
                return Err(CorePreludeImportError::ForeignBinding {
                    binding,
                    exporter: key.exporter(),
                });
            }
            let identity = self
                .identity(binding)
                .ok_or(CorePreludeImportError::MissingBindingIdentity(binding))?;
            bindings.push(ImportedCorePreludeBinding {
                identity,
                key,
                target,
            });
        }

        Ok(ImportedHirSet {
            foundation: self,
            interface,
            bindings,
            capability: PhantomData,
        })
    }
}

impl WireEncode for ImportedHirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

/// Marker for the core type bindings used during HIR lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeOnly {}

/// Imported type bindings used during HIR lowering. Compiler protocols own
/// their identities; function and value lookup uses the shared dependency world.
pub struct ImportedHirSet<'a, Capability> {
    foundation: &'a ImportedHirFoundation,
    interface: &'a CoreHirInterfaceV1,
    bindings: Vec<ImportedCorePreludeBinding<'a>>,
    capability: PhantomData<fn() -> Capability>,
}

impl<'a> ImportedHirSet<'a, CorePreludeOnly> {
    pub(super) const fn core_interface(&self) -> &'a CoreHirInterfaceV1 {
        self.interface
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.foundation.origin()
    }

    pub fn bindings(&self) -> &[ImportedCorePreludeBinding<'a>] {
        &self.bindings
    }

    pub fn candidates<'set>(
        &'set self,
        namespace: BindingNamespace,
        name: &'set str,
    ) -> impl Iterator<Item = &'set ImportedCorePreludeBinding<'a>> + 'set {
        self.bindings.iter().filter(move |binding| {
            binding.key.namespace() == namespace && binding.key.name().as_str() == name
        })
    }
}

pub struct ImportedCorePreludeBinding<'a> {
    identity: ImportedHirId<PersistentExportBindingId>,
    key: &'a ExportBindingKey,
    target: &'a CoreTypeTargetV1,
}

impl<'a> ImportedCorePreludeBinding<'a> {
    pub const fn identity(&self) -> ImportedHirId<PersistentExportBindingId> {
        self.identity
    }

    pub const fn key(&self) -> &'a ExportBindingKey {
        self.key
    }

    pub const fn target(&self) -> &'a CoreTypeTargetV1 {
        self.target
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeImportError {
    FoundationNotCore(ConeIdentity),
    MissingBindingKey(PersistentExportBindingId),
    ForeignBinding {
        binding: PersistentExportBindingId,
        exporter: ConeIdentity,
    },
    MissingBindingIdentity(PersistentExportBindingId),
}

impl fmt::Display for CorePreludeImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot import trusted core prelude: {self:?}")
    }
}

impl std::error::Error for CorePreludeImportError {}
