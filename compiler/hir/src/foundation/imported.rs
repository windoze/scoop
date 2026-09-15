use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_identity::{
    BindingNamespace, ConeIdentity, ExportBindingKey, HirIdentityLayer, ImportedIdentityId,
    ImportedIdentityMap, PersistentEnumVariantFieldId, PersistentEnumVariantId,
    PersistentExactTypeId, PersistentExportBindingId, PersistentId, PersistentTypeId,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalHirFoundation, HirFoundationCounts, OdrFreeHirFoundation, ValidatedHirFoundation,
};
use crate::{
    CoreCallableDefinitionV1, CoreCallableTargetV1, CoreHirCallableCapabilityV1,
    CoreHirInterfaceV1, CoreTypeTargetV1, CoreValueTargetV1,
};

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
    pub(super) fn import_core_prelude<'a>(
        &'a self,
        interface: &'a CoreHirInterfaceV1,
        strong_callable_bindings: &'a [PersistentExportBindingId],
    ) -> Result<ImportedHirSet<'a, CorePreludeOnly>, CorePreludeImportError> {
        if self.origin() != ConeIdentity::CORE {
            return Err(CorePreludeImportError::FoundationNotCore(self.origin()));
        }
        validate_strong_callable_bindings(interface, strong_callable_bindings)?;

        let projection = next_imported_core_prelude_projection();
        let mut bindings = Vec::with_capacity(
            interface
                .prelude_snapshot()
                .ordinary_bindings()
                .bindings()
                .len(),
        );
        for (index, &binding) in interface
            .prelude_snapshot()
            .ordinary_bindings()
            .bindings()
            .iter()
            .enumerate()
        {
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
                reference: ImportedCorePreludeRef {
                    projection,
                    binding: checked_selection_index(index),
                },
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
            interface,
            strong_callable_bindings,
            projection,
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
    interface: &'a CoreHirInterfaceV1,
    strong_callable_bindings: &'a [PersistentExportBindingId],
    projection: ImportedCorePreludeProjectionId,
    bindings: Vec<ImportedCorePreludeBinding<'a>>,
    option_some: ImportedHirId<PersistentEnumVariantId>,
    option_some_payload: ImportedHirId<PersistentEnumVariantFieldId>,
    option_none: ImportedHirId<PersistentEnumVariantId>,
    string_source: ImportedHirId<PersistentTypeId>,
    string_exact: ImportedHirId<PersistentExactTypeId>,
    capability: PhantomData<fn() -> Capability>,
}

impl<'a> ImportedHirSet<'a, CorePreludeOnly> {
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

    /// Starts a cloneable, lifetime-free selection transaction for this
    /// exact prelude projection. Candidate probing may clone and discard this
    /// plan without retaining a borrow of the artifact; only `bind_selection`
    /// can turn the committed plan into the artifact-borrowing sidecar.
    pub fn selection_plan(&self) -> ImportedCoreSelectionPlan {
        ImportedCoreSelectionPlan {
            projection: self.projection,
            selection: next_imported_core_selection(),
            entries: self
                .bindings
                .iter()
                .map(|binding| ImportedCoreSelectionPlanEntry {
                    identity: binding.identity,
                    target: binding.target.into(),
                    has_strong_callable_implementation: self
                        .strong_callable_bindings
                        .binary_search(&binding.identity.persistent())
                        .is_ok(),
                })
                .collect(),
            by_binding: BTreeMap::new(),
            callables: Vec::new(),
            types: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Binds one committed selection transaction back to the exact imported
    /// artifact that minted its unforgeable prelude references.
    pub fn bind_selection(
        &self,
        plan: ImportedCoreSelectionPlan,
    ) -> Result<SelectedImportedCoreSet<'a>, CorePreludeSelectionBindError> {
        if plan.projection != self.projection {
            return Err(CorePreludeSelectionBindError::ForeignProjection);
        }
        let target = |index: u32| {
            let binding = self
                .bindings
                .get(index as usize)
                .expect("a selection plan only contains bindings from its prelude projection");
            SelectedImportedCoreTarget {
                foundation: self.foundation,
                interface: self.interface,
                strong_callable_bindings: self.strong_callable_bindings,
                binding: binding.identity,
                target: binding.target,
            }
        };
        Ok(SelectedImportedCoreSet {
            foundation: self.foundation,
            interface: self.interface,
            strong_callable_bindings: self.strong_callable_bindings,
            selection: plan.selection,
            callables: plan
                .callables
                .into_iter()
                .map(|index| (ImportedCoreCallableId(index), target(index)))
                .collect(),
            types: plan
                .types
                .into_iter()
                .map(|index| (ImportedCoreTypeId(index), target(index)))
                .collect(),
            values: plan
                .values
                .into_iter()
                .map(|index| (ImportedCoreValueId(index), target(index)))
                .collect(),
        })
    }
}

pub struct ImportedCorePreludeBinding<'a> {
    reference: ImportedCorePreludeRef,
    identity: ImportedHirId<PersistentExportBindingId>,
    key: &'a ExportBindingKey,
    target: ImportedCorePreludeTarget<'a>,
}

impl<'a> ImportedCorePreludeBinding<'a> {
    pub const fn reference(&self) -> ImportedCorePreludeRef {
        self.reference
    }

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

#[derive(Clone, Copy, Debug)]
pub enum ImportedCorePreludeTarget<'a> {
    Callable(&'a CoreCallableTargetV1),
    Type(&'a CoreTypeTargetV1),
    Value(&'a CoreValueTargetV1),
}

#[derive(Clone, Debug)]
enum OwnedImportedCorePreludeTarget {
    Callable(CoreCallableTargetV1),
    Type(CoreTypeTargetV1),
    Value(CoreValueTargetV1),
}

impl From<ImportedCorePreludeTarget<'_>> for OwnedImportedCorePreludeTarget {
    fn from(target: ImportedCorePreludeTarget<'_>) -> Self {
        match target {
            ImportedCorePreludeTarget::Callable(target) => Self::Callable(target.clone()),
            ImportedCorePreludeTarget::Type(target) => Self::Type(target.clone()),
            ImportedCorePreludeTarget::Value(target) => Self::Value(target.clone()),
        }
    }
}

/// Unforgeable, lifetime-free reference to one candidate in an imported core
/// prelude projection. It is safe to retain in cloneable frontend probe state;
/// another projection cannot resolve it even when the numeric index matches.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedCorePreludeRef {
    projection: ImportedCorePreludeProjectionId,
    binding: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ImportedCorePreludeProjectionId(u64);

fn next_imported_core_prelude_projection() -> ImportedCorePreludeProjectionId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let projection = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("the imported core prelude projection id space is exhausted");
    ImportedCorePreludeProjectionId(projection)
}

#[derive(Clone, Debug)]
struct ImportedCoreSelectionPlanEntry {
    identity: ImportedHirId<PersistentExportBindingId>,
    target: OwnedImportedCorePreludeTarget,
    has_strong_callable_implementation: bool,
}

/// Cloneable frontend transaction for imported-core selection.
///
/// It deliberately owns only validated candidate snapshots and process-local
/// brands. Artifact-borrowing target proofs appear only after the winning
/// transaction is bound through `ImportedHirSet::bind_selection`.
#[derive(Clone, Debug)]
pub struct ImportedCoreSelectionPlan {
    projection: ImportedCorePreludeProjectionId,
    selection: ImportedCoreSelectionId,
    entries: Vec<ImportedCoreSelectionPlanEntry>,
    by_binding: BTreeMap<PersistentExportBindingId, SelectedImportedCoreIndex>,
    callables: Vec<u32>,
    types: Vec<u32>,
    values: Vec<u32>,
}

impl ImportedCoreSelectionPlan {
    pub fn select(
        &mut self,
        reference: ImportedCorePreludeRef,
    ) -> Result<SelectedImportedCoreId, CorePreludeSelectionError> {
        if reference.projection != self.projection {
            return Err(CorePreludeSelectionError::ForeignProjection);
        }
        let entry = self
            .entries
            .get(reference.binding as usize)
            .cloned()
            .expect("a prelude reference is minted with an in-range binding index");
        if let Some(&selected) = self.by_binding.get(&entry.identity.persistent()) {
            return Ok(self.reference(selected));
        }
        let unavailable = match &entry.target {
            OwnedImportedCorePreludeTarget::Callable(target) => callable_unavailability(
                target.capability(),
                entry.has_strong_callable_implementation,
            ),
            OwnedImportedCorePreludeTarget::Type(target) => match target.capability() {
                crate::CoreHirTypeCapabilityV1::ParamFreeStrong(_) => None,
                crate::CoreHirTypeCapabilityV1::StructuralUnavailable(_) => {
                    Some(CorePreludeUnavailableCapability::Structural)
                }
                crate::CoreHirTypeCapabilityV1::GenericUnavailable { .. } => {
                    Some(CorePreludeUnavailableCapability::Generic)
                }
            },
            OwnedImportedCorePreludeTarget::Value(target) => match target.capability() {
                crate::CoreHirValueCapabilityV1::ParamFreeStrong(_) => None,
                crate::CoreHirValueCapabilityV1::StructuralUnavailable(_) => {
                    Some(CorePreludeUnavailableCapability::Structural)
                }
                crate::CoreHirValueCapabilityV1::GenericUnavailable { .. } => {
                    Some(CorePreludeUnavailableCapability::Generic)
                }
            },
        };
        if let Some(required) = unavailable {
            return Err(CorePreludeSelectionError::Capability(
                CorePreludeCapabilityError {
                    binding: entry.identity,
                    required,
                },
            ));
        }
        let selected = match entry.target {
            OwnedImportedCorePreludeTarget::Callable(_) => {
                let id = ImportedCoreCallableId(reference.binding);
                self.callables.push(reference.binding);
                SelectedImportedCoreIndex::Callable(id)
            }
            OwnedImportedCorePreludeTarget::Type(_) => {
                let id = ImportedCoreTypeId(reference.binding);
                self.types.push(reference.binding);
                SelectedImportedCoreIndex::Type(id)
            }
            OwnedImportedCorePreludeTarget::Value(_) => {
                let id = ImportedCoreValueId(reference.binding);
                self.values.push(reference.binding);
                SelectedImportedCoreIndex::Value(id)
            }
        };
        let previous = self
            .by_binding
            .insert(entry.identity.persistent(), selected);
        assert!(previous.is_none(), "a fresh imported binding is unique");
        Ok(self.reference(selected))
    }

    fn reference(&self, selected: SelectedImportedCoreIndex) -> SelectedImportedCoreId {
        match selected {
            SelectedImportedCoreIndex::Callable(callable) => {
                SelectedImportedCoreId::Callable(ImportedCoreCallableRef {
                    selection: self.selection,
                    callable,
                })
            }
            SelectedImportedCoreIndex::Type(ty) => {
                SelectedImportedCoreId::Type(ImportedCoreTypeRef {
                    selection: self.selection,
                    ty,
                })
            }
            SelectedImportedCoreIndex::Value(value) => {
                SelectedImportedCoreId::Value(ImportedCoreValueRef {
                    selection: self.selection,
                    value,
                })
            }
        }
    }
}

/// A core prelude target proven usable by the M23-3 param-free strong path.
///
/// Its fields are private so a raw lookup candidate or persistent id cannot
/// be promoted without checking the capability attached by the trusted core
/// artifact.
#[derive(Clone, Copy)]
pub struct SelectedImportedCoreTarget<'a> {
    foundation: &'a ImportedHirFoundation,
    interface: &'a CoreHirInterfaceV1,
    strong_callable_bindings: &'a [PersistentExportBindingId],
    binding: ImportedHirId<PersistentExportBindingId>,
    target: ImportedCorePreludeTarget<'a>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ImportedCoreCallableId(u32);

/// One callable reference branded by the exact selected-set world that
/// admitted it. Two lowering requests may both allocate callable index zero;
/// this value keeps those otherwise equal indices non-interchangeable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedCoreCallableRef {
    selection: ImportedCoreSelectionId,
    callable: ImportedCoreCallableId,
}

/// Process-local brand for one selected imported-core world.
///
/// It is neither a wire identity nor a persistent semantic identity. The
/// private representation ensures only this module can mint brands.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ImportedCoreSelectionId(u64);

fn next_imported_core_selection() -> ImportedCoreSelectionId {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let selection = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("the imported core selection id space is exhausted");
    ImportedCoreSelectionId(selection)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ImportedCoreTypeId(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ImportedCoreValueId(u32);

/// One selected type branded by the exact request-local world that admitted
/// it. The internal arena index is never exposed independently.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedCoreTypeRef {
    selection: ImportedCoreSelectionId,
    ty: ImportedCoreTypeId,
}

/// One selected value branded by the exact request-local world that admitted
/// it. The internal arena index is never exposed independently.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedCoreValueRef {
    selection: ImportedCoreSelectionId,
    value: ImportedCoreValueId,
}

/// Typed result of admitting one checked prelude binding into the current
/// lowering session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedImportedCoreId {
    Callable(ImportedCoreCallableRef),
    Type(ImportedCoreTypeRef),
    Value(ImportedCoreValueRef),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectedImportedCoreIndex {
    Callable(ImportedCoreCallableId),
    Type(ImportedCoreTypeId),
    Value(ImportedCoreValueId),
}

/// The only selected imported HIR targets visible to one ordinary lowering.
///
/// Entries retain the borrowed selection proof minted by the trusted core
/// artifact. The three id domains prevent a callable, type, or value from
/// being substituted for another entity kind. Their private indices are the
/// stable positions in the exact prelude projection, so cloned transactions
/// cannot alias different targets by selecting them in a different order.
pub struct SelectedImportedCoreSet<'a> {
    foundation: &'a ImportedHirFoundation,
    interface: &'a CoreHirInterfaceV1,
    strong_callable_bindings: &'a [PersistentExportBindingId],
    selection: ImportedCoreSelectionId,
    callables: BTreeMap<ImportedCoreCallableId, SelectedImportedCoreTarget<'a>>,
    types: BTreeMap<ImportedCoreTypeId, SelectedImportedCoreTarget<'a>>,
    values: BTreeMap<ImportedCoreValueId, SelectedImportedCoreTarget<'a>>,
}

impl<'a> SelectedImportedCoreSet<'a> {
    /// Resolves a branded callable only when it was minted by this exact set.
    pub fn resolve_callable(
        &self,
        reference: ImportedCoreCallableRef,
    ) -> Option<SelectedImportedCoreTarget<'a>> {
        (reference.selection == self.selection)
            .then(|| self.callables.get(&reference.callable).copied())
            .flatten()
    }

    pub fn resolve_type(
        &self,
        reference: ImportedCoreTypeRef,
    ) -> Option<SelectedImportedCoreTarget<'a>> {
        (reference.selection == self.selection)
            .then(|| self.types.get(&reference.ty).copied())
            .flatten()
    }

    pub fn resolve_value(
        &self,
        reference: ImportedCoreValueRef,
    ) -> Option<SelectedImportedCoreTarget<'a>> {
        (reference.selection == self.selection)
            .then(|| self.values.get(&reference.value).copied())
            .flatten()
    }

    pub fn callable_count(&self) -> usize {
        self.callables.len()
    }

    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    pub fn value_count(&self) -> usize {
        self.values.len()
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedHirFoundation,
        interface: &CoreHirInterfaceV1,
        strong_callable_bindings: &[PersistentExportBindingId],
    ) -> bool {
        std::ptr::eq(self.foundation, foundation)
            && std::ptr::eq(self.interface, interface)
            && std::ptr::eq(self.strong_callable_bindings, strong_callable_bindings)
    }

    #[doc(hidden)]
    pub fn callable_selections(&self) -> impl Iterator<Item = SelectedImportedCoreTarget<'a>> + '_ {
        self.callables.values().copied()
    }
}

fn checked_selection_index(length: usize) -> u32 {
    u32::try_from(length).expect("one HIR request cannot select more than u32::MAX core bindings")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeSelectionError {
    ForeignProjection,
    Capability(CorePreludeCapabilityError),
}

impl fmt::Display for CorePreludeSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignProjection => {
                formatter.write_str("imported core reference belongs to another prelude projection")
            }
            Self::Capability(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CorePreludeSelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ForeignProjection => None,
            Self::Capability(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeSelectionBindError {
    ForeignProjection,
}

impl fmt::Display for CorePreludeSelectionBindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignProjection => {
                formatter.write_str("imported core selection belongs to another prelude projection")
            }
        }
    }
}

impl std::error::Error for CorePreludeSelectionBindError {}

impl<'a> SelectedImportedCoreTarget<'a> {
    pub const fn binding(self) -> ImportedHirId<PersistentExportBindingId> {
        self.binding
    }

    pub const fn target(self) -> ImportedCorePreludeTarget<'a> {
        self.target
    }

    #[doc(hidden)]
    pub fn belongs_to(
        self,
        foundation: &ImportedHirFoundation,
        interface: &CoreHirInterfaceV1,
        strong_callable_bindings: &[PersistentExportBindingId],
    ) -> bool {
        std::ptr::eq(self.foundation, foundation)
            && std::ptr::eq(self.interface, interface)
            && std::ptr::eq(self.strong_callable_bindings, strong_callable_bindings)
    }
}

impl fmt::Debug for SelectedImportedCoreTarget<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedImportedCoreTarget")
            .field("binding", &self.binding)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeUnavailableCapability {
    Implementation,
    Structural,
    Generic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CorePreludeCapabilityError {
    binding: ImportedHirId<PersistentExportBindingId>,
    required: CorePreludeUnavailableCapability,
}

impl CorePreludeCapabilityError {
    pub const fn binding(self) -> ImportedHirId<PersistentExportBindingId> {
        self.binding
    }

    pub const fn required(self) -> CorePreludeUnavailableCapability {
        self.required
    }

    pub const fn code(self) -> &'static str {
        capability_error_code(self.required)
    }
}

impl fmt::Display for CorePreludeCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: imported core binding requires unavailable {:?} materialization",
            self.code(),
            self.required
        )
    }
}

impl std::error::Error for CorePreludeCapabilityError {}

const fn capability_error_code(required: CorePreludeUnavailableCapability) -> &'static str {
    match required {
        CorePreludeUnavailableCapability::Implementation => {
            "SCOOPC_CAPABILITY_CORE_IMPLEMENTATION_UNAVAILABLE"
        }
        CorePreludeUnavailableCapability::Structural
        | CorePreludeUnavailableCapability::Generic => "SCOOPC_CAPABILITY_CORE_GENERIC_UNAVAILABLE",
    }
}

fn callable_unavailability(
    capability: &CoreHirCallableCapabilityV1,
    has_strong_implementation: bool,
) -> Option<CorePreludeUnavailableCapability> {
    match capability {
        CoreHirCallableCapabilityV1::ParamFreeCandidate(_) if has_strong_implementation => None,
        CoreHirCallableCapabilityV1::ParamFreeCandidate(_) => {
            Some(CorePreludeUnavailableCapability::Implementation)
        }
        CoreHirCallableCapabilityV1::StructuralUnavailable(_) => {
            Some(CorePreludeUnavailableCapability::Structural)
        }
        CoreHirCallableCapabilityV1::GenericUnavailable { .. } => {
            Some(CorePreludeUnavailableCapability::Generic)
        }
    }
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

fn validate_strong_callable_bindings(
    interface: &CoreHirInterfaceV1,
    bindings: &[PersistentExportBindingId],
) -> Result<(), CorePreludeImportError> {
    if bindings.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CorePreludeImportError::NonCanonicalStrongCallableBindings);
    }
    for &binding in bindings {
        let Some(target) = interface
            .callable_targets()
            .targets()
            .iter()
            .find(|target| target.binding() == binding)
        else {
            return Err(CorePreludeImportError::UnknownStrongCallableBinding(
                binding,
            ));
        };
        if !matches!(target.definition(), CoreCallableDefinitionV1::Function(_))
            || !matches!(
                target.capability(),
                CoreHirCallableCapabilityV1::ParamFreeCandidate(_)
            )
        {
            return Err(CorePreludeImportError::IneligibleStrongCallableBinding(
                binding,
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeImportError {
    FoundationNotCore(ConeIdentity),
    NonCanonicalStrongCallableBindings,
    UnknownStrongCallableBinding(PersistentExportBindingId),
    IneligibleStrongCallableBinding(PersistentExportBindingId),
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

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CoreBuiltinNominal, Effect, ExactCallableSignature, ExactTypeKey, PersistentExactTypeId,
    };

    use super::*;

    #[test]
    fn callable_candidate_requires_the_joined_mir_implementation_proof() {
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let candidate = CoreHirCallableCapabilityV1::ParamFreeCandidate(
            ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
        );

        assert_eq!(callable_unavailability(&candidate, true), None);
        assert_eq!(
            callable_unavailability(&candidate, false),
            Some(CorePreludeUnavailableCapability::Implementation)
        );
        assert_eq!(
            capability_error_code(CorePreludeUnavailableCapability::Implementation),
            "SCOOPC_CAPABILITY_CORE_IMPLEMENTATION_UNAVAILABLE"
        );
    }

    #[test]
    fn selected_imported_worlds_have_distinct_process_local_brands() {
        assert_ne!(
            next_imported_core_selection(),
            next_imported_core_selection()
        );
    }
}
