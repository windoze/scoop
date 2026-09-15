use std::collections::BTreeMap;
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
    pub fn import_core_prelude<'a>(
        &'a self,
        interface: &'a CoreHirInterfaceV1,
        strong_callable_bindings: &'a [PersistentExportBindingId],
    ) -> Result<ImportedHirSet<'a, CorePreludeOnly>, CorePreludeImportError> {
        if self.origin() != ConeIdentity::CORE {
            return Err(CorePreludeImportError::FoundationNotCore(self.origin()));
        }
        validate_strong_callable_bindings(interface, strong_callable_bindings)?;

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
                foundation: self,
                interface,
                strong_callable_bindings,
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

    /// Starts the unique selected-target sidecar for this exact core
    /// projection. There is no detached constructor that could combine
    /// candidates from different artifacts.
    pub fn selected_set(&self) -> SelectedImportedCoreSet<'a> {
        SelectedImportedCoreSet {
            foundation: self.foundation,
            interface: self.interface,
            strong_callable_bindings: self.strong_callable_bindings,
            by_binding: BTreeMap::new(),
            callables: Vec::new(),
            types: Vec::new(),
            values: Vec::new(),
        }
    }
}

pub struct ImportedCorePreludeBinding<'a> {
    foundation: &'a ImportedHirFoundation,
    interface: &'a CoreHirInterfaceV1,
    strong_callable_bindings: &'a [PersistentExportBindingId],
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

    fn belongs_to(
        &self,
        foundation: &ImportedHirFoundation,
        interface: &CoreHirInterfaceV1,
        strong_callable_bindings: &[PersistentExportBindingId],
    ) -> bool {
        std::ptr::eq(self.foundation, foundation)
            && std::ptr::eq(self.interface, interface)
            && std::ptr::eq(self.strong_callable_bindings, strong_callable_bindings)
    }

    /// Selects this lookup candidate for M23-3 lowering.
    ///
    /// Lookup deliberately exposes every checked prelude declaration so an
    /// unavailable declaration can participate in diagnostics. Only this
    /// method can turn a candidate into a selected external target, and it
    /// rejects every capability that would require generic or structural
    /// materialization.
    pub fn select_param_free_strong(
        &self,
    ) -> Result<SelectedImportedCoreTarget<'a>, CorePreludeCapabilityError> {
        let unavailable = match self.target {
            ImportedCorePreludeTarget::Callable(target) => callable_unavailability(
                target.capability(),
                self.strong_callable_bindings
                    .binary_search(&self.identity.persistent())
                    .is_ok(),
            ),
            ImportedCorePreludeTarget::Type(target) => match target.capability() {
                crate::CoreHirTypeCapabilityV1::ParamFreeStrong(_) => None,
                crate::CoreHirTypeCapabilityV1::StructuralUnavailable(_) => {
                    Some(CorePreludeUnavailableCapability::Structural)
                }
                crate::CoreHirTypeCapabilityV1::GenericUnavailable { .. } => {
                    Some(CorePreludeUnavailableCapability::Generic)
                }
            },
            ImportedCorePreludeTarget::Value(target) => match target.capability() {
                crate::CoreHirValueCapabilityV1::ParamFreeStrong(_) => None,
                crate::CoreHirValueCapabilityV1::StructuralUnavailable(_) => {
                    Some(CorePreludeUnavailableCapability::Structural)
                }
                crate::CoreHirValueCapabilityV1::GenericUnavailable { .. } => {
                    Some(CorePreludeUnavailableCapability::Generic)
                }
            },
        };
        match unavailable {
            Some(required) => Err(CorePreludeCapabilityError {
                binding: self.identity,
                required,
            }),
            None => Ok(SelectedImportedCoreTarget {
                foundation: self.foundation,
                interface: self.interface,
                strong_callable_bindings: self.strong_callable_bindings,
                binding: self.identity,
                target: self.target,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ImportedCorePreludeTarget<'a> {
    Callable(&'a CoreCallableTargetV1),
    Type(&'a CoreTypeTargetV1),
    Value(&'a CoreValueTargetV1),
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

/// Request-local id of one selected callable from the trusted core prelude.
///
/// This id is meaningful only inside the [`SelectedImportedCoreSet`] that
/// minted it. It is deliberately distinct from source declarations,
/// persistent function identities, and imported MIR/LIR identities.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportedCoreCallableId(u32);

/// Request-local id of one selected type from the trusted core prelude.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportedCoreTypeId(u32);

/// Request-local id of one selected value from the trusted core prelude.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportedCoreValueId(u32);

/// Typed result of admitting one checked prelude binding into the current
/// lowering session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedImportedCoreId {
    Callable(ImportedCoreCallableId),
    Type(ImportedCoreTypeId),
    Value(ImportedCoreValueId),
}

/// The only selected imported HIR targets visible to one ordinary lowering.
///
/// Entries retain the borrowed selection proof minted by the trusted core
/// artifact. The three id domains prevent a callable, type, or value from
/// being substituted for another entity kind, while the private maps make
/// repeated lookup of the same binding converge on one request-local id.
pub struct SelectedImportedCoreSet<'a> {
    foundation: &'a ImportedHirFoundation,
    interface: &'a CoreHirInterfaceV1,
    strong_callable_bindings: &'a [PersistentExportBindingId],
    by_binding: BTreeMap<PersistentExportBindingId, SelectedImportedCoreId>,
    callables: Vec<SelectedImportedCoreTarget<'a>>,
    types: Vec<SelectedImportedCoreTarget<'a>>,
    values: Vec<SelectedImportedCoreTarget<'a>>,
}

impl<'a> SelectedImportedCoreSet<'a> {
    /// Selects and interns one raw lookup candidate. Capability rejection
    /// happens before the set changes, so failed selection cannot leave a
    /// partial imported world.
    pub fn select(
        &mut self,
        binding: &ImportedCorePreludeBinding<'a>,
    ) -> Result<SelectedImportedCoreId, CorePreludeSelectionError> {
        if !binding.belongs_to(
            self.foundation,
            self.interface,
            self.strong_callable_bindings,
        ) {
            return Err(CorePreludeSelectionError::ForeignBinding(
                binding.identity().persistent(),
            ));
        }
        if let Some(&selected) = self.by_binding.get(&binding.identity().persistent()) {
            return Ok(selected);
        }
        let target = binding
            .select_param_free_strong()
            .map_err(CorePreludeSelectionError::Capability)?;
        let selected = match target.target() {
            ImportedCorePreludeTarget::Callable(_) => {
                let id = ImportedCoreCallableId(checked_selection_index(self.callables.len()));
                self.callables.push(target);
                SelectedImportedCoreId::Callable(id)
            }
            ImportedCorePreludeTarget::Type(_) => {
                let id = ImportedCoreTypeId(checked_selection_index(self.types.len()));
                self.types.push(target);
                SelectedImportedCoreId::Type(id)
            }
            ImportedCorePreludeTarget::Value(_) => {
                let id = ImportedCoreValueId(checked_selection_index(self.values.len()));
                self.values.push(target);
                SelectedImportedCoreId::Value(id)
            }
        };
        let previous = self
            .by_binding
            .insert(binding.identity().persistent(), selected);
        assert!(previous.is_none(), "a fresh imported binding is unique");
        Ok(selected)
    }

    pub fn callable(&self, id: ImportedCoreCallableId) -> Option<SelectedImportedCoreTarget<'a>> {
        self.callables.get(id.0 as usize).copied()
    }

    pub fn ty(&self, id: ImportedCoreTypeId) -> Option<SelectedImportedCoreTarget<'a>> {
        self.types.get(id.0 as usize).copied()
    }

    pub fn value(&self, id: ImportedCoreValueId) -> Option<SelectedImportedCoreTarget<'a>> {
        self.values.get(id.0 as usize).copied()
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
        self.callables.iter().copied()
    }
}

fn checked_selection_index(length: usize) -> u32 {
    u32::try_from(length).expect("one HIR request cannot select more than u32::MAX core bindings")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeSelectionError {
    ForeignBinding(PersistentExportBindingId),
    Capability(CorePreludeCapabilityError),
}

impl fmt::Display for CorePreludeSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignBinding(binding) => write!(
                formatter,
                "imported core binding {binding} belongs to another prelude projection"
            ),
            Self::Capability(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CorePreludeSelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ForeignBinding(_) => None,
            Self::Capability(error) => Some(error),
        }
    }
}

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
}
