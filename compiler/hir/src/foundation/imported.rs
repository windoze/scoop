use std::fmt;
use std::marker::PhantomData;

use scoop_identity::{
    BindingNamespace, ConeIdentity, ExportBindingKey, HirIdentityLayer, ImportedIdentityId,
    ImportedIdentityMap, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExportBindingId, PersistentId, PersistentTypeId,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalHirFoundation, HirFoundationCounts, OdrFreeHirFoundation, ValidatedHirFoundation,
};
use crate::{CoreCallableTargetV1, CoreHirInterfaceV1, CoreTypeTargetV1, CoreValueTargetV1};

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

    /// Restricts one already imported core foundation to the M23-3 prelude
    /// capability. The resulting value has no API for ordinary package,
    /// exact-import, star-import, or re-export enumeration.
    pub fn import_core_prelude<'a>(
        &'a self,
        interface: &'a CoreHirInterfaceV1,
    ) -> Result<ImportedHirSet<'a, CorePreludeOnly>, CorePreludeImportError> {
        if self.origin() != ConeIdentity::CORE {
            return Err(CorePreludeImportError::FoundationNotCore(self.origin()));
        }

        let mut bindings = Vec::with_capacity(
            interface
                .prelude_snapshot()
                .ordinary_bindings()
                .bindings()
                .len(),
        );
        for &binding in interface.prelude_snapshot().ordinary_bindings().bindings() {
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
            let target = core_prelude_target(interface, binding)?;
            bindings.push(ImportedCorePreludeBinding {
                identity,
                key,
                target,
            });
        }

        let snapshot = interface.prelude_snapshot();
        let option_some = self
            .identity(snapshot.option_some())
            .ok_or(CorePreludeImportError::MissingOptionSome)?;
        let option_some_payload = self
            .identity(snapshot.option_some_payload())
            .ok_or(CorePreludeImportError::MissingOptionSomePayload)?;
        let option_none = self
            .identity(snapshot.option_none())
            .ok_or(CorePreludeImportError::MissingOptionNone)?;
        let string = interface.string_capability();
        let string_source = self
            .identity(string.source_type())
            .ok_or(CorePreludeImportError::MissingStringSource)?;
        let string_exact = self
            .identity(string.exact_type())
            .ok_or(CorePreludeImportError::MissingStringExact)?;

        Ok(ImportedHirSet {
            foundation: self,
            bindings,
            option_some,
            option_some_payload,
            option_none,
            string_source,
            string_exact,
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

/// Marker for the only cross-Cone HIR capability admitted by M23-3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeOnly {}

/// A capability-restricted projection of an imported HIR foundation.
///
/// The generic marker prevents this value from being confused with future
/// ordinary dependency views. Fields are private so only the checked
/// projection above can bind an interface to its imported identity session.
pub struct ImportedHirSet<'a, Capability> {
    foundation: &'a ImportedHirFoundation,
    bindings: Vec<ImportedCorePreludeBinding<'a>>,
    option_some: ImportedHirId<PersistentEnumVariantId>,
    option_some_payload: ImportedHirId<PersistentEnumVariantFieldId>,
    option_none: ImportedHirId<PersistentEnumVariantId>,
    string_source: ImportedHirId<PersistentTypeId>,
    string_exact: ImportedHirId<PersistentExactTypeId>,
    capability: PhantomData<fn() -> Capability>,
}

impl ImportedHirSet<'_, CorePreludeOnly> {
    pub const fn origin(&self) -> ConeIdentity {
        self.foundation.origin()
    }

    pub fn bindings(&self) -> &[ImportedCorePreludeBinding<'_>] {
        &self.bindings
    }

    pub fn candidates<'set>(
        &'set self,
        namespace: BindingNamespace,
        name: &'set str,
    ) -> impl Iterator<Item = &'set ImportedCorePreludeBinding<'set>> + 'set {
        self.bindings.iter().filter(move |binding| {
            binding.key.namespace() == namespace && binding.key.name().as_str() == name
        })
    }

    pub const fn option_some(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.option_some
    }

    pub const fn option_some_payload(&self) -> ImportedHirId<PersistentEnumVariantFieldId> {
        self.option_some_payload
    }

    pub const fn option_none(&self) -> ImportedHirId<PersistentEnumVariantId> {
        self.option_none
    }

    pub const fn string_source(&self) -> ImportedHirId<PersistentTypeId> {
        self.string_source
    }

    pub const fn string_exact(&self) -> ImportedHirId<PersistentExactTypeId> {
        self.string_exact
    }
}

pub struct ImportedCorePreludeBinding<'a> {
    identity: ImportedHirId<PersistentExportBindingId>,
    key: &'a ExportBindingKey,
    target: ImportedCorePreludeTarget<'a>,
}

impl<'a> ImportedCorePreludeBinding<'a> {
    pub const fn identity(&self) -> ImportedHirId<PersistentExportBindingId> {
        self.identity
    }

    pub const fn key(&self) -> &'a ExportBindingKey {
        self.key
    }

    pub const fn target(&self) -> ImportedCorePreludeTarget<'a> {
        self.target
    }
}

#[derive(Clone, Copy)]
pub enum ImportedCorePreludeTarget<'a> {
    Callable(&'a CoreCallableTargetV1),
    Type(&'a CoreTypeTargetV1),
    Value(&'a CoreValueTargetV1),
}

fn core_prelude_target(
    interface: &CoreHirInterfaceV1,
    binding: PersistentExportBindingId,
) -> Result<ImportedCorePreludeTarget<'_>, CorePreludeImportError> {
    let callable = interface
        .callable_targets()
        .targets()
        .iter()
        .find(|target| target.binding() == binding);
    let ty = interface
        .type_targets()
        .targets()
        .iter()
        .find(|target| target.binding() == binding);
    let value = interface
        .value_targets()
        .targets()
        .iter()
        .find(|target| target.binding() == binding);
    match (callable, ty, value) {
        (Some(target), None, None) => Ok(ImportedCorePreludeTarget::Callable(target)),
        (None, Some(target), None) => Ok(ImportedCorePreludeTarget::Type(target)),
        (None, None, Some(target)) => Ok(ImportedCorePreludeTarget::Value(target)),
        (None, None, None) => Err(CorePreludeImportError::MissingConstituent(binding)),
        _ => Err(CorePreludeImportError::DuplicateConstituent(binding)),
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
    MissingConstituent(PersistentExportBindingId),
    DuplicateConstituent(PersistentExportBindingId),
    MissingOptionSome,
    MissingOptionSomePayload,
    MissingOptionNone,
    MissingStringSource,
    MissingStringExact,
}

impl fmt::Display for CorePreludeImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot import trusted core prelude: {self:?}")
    }
}

impl std::error::Error for CorePreludeImportError {}
