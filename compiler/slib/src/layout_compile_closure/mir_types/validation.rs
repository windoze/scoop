use scoop_hir as hir;
use scoop_identity::{
    CoreBuiltinNominal, PersistentExactTypeId, PersistentTypeId, SignatureTypeKey,
};
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::{Error, SharedMirTypeComponent as Component};

/// Joins source declarations and actual generated shapes to the MIR type table.
/// This does not validate callables, dispatch, selected uses or machine layouts.
pub fn validate_shared_mir_type_exports(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[hir::CheckedSharedTypeFoundationV1<'_>],
    inheritance: &hir::CheckedNominalInheritanceGraphV1<'_>,
    core: &hir::CoreBootstrapInterfaceSectionV1,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    payload_types: &dyn mir::MirTypeBridgeTypeLookupV1,
    shapes: &mir::CanonicalMirShapeSupportsV1,
) -> Result<(), Error> {
    let mut comparison = Comparison {
        source,
        inheritance,
        types,
        payload_types,

        expected: Vec::new(),
    };
    for representation in source.representations().table().records() {
        let owner = representation.owner();
        let exact = comparison.exact(owner)?;
        let record = comparison.require_type(exact)?;
        Error::require(
            exact,
            Component::Origin,
            record.origin() == &mir::MirTypeOriginV1::SourceNominal(owner),
        )?;
        comparison.facts(exact, record)?;
        comparison.inheritance(exact, record)?;
        super::representation::validate(&mut comparison, representation, record)?;
    }
    for record in types.records() {
        if matches!(record.origin(), mir::MirTypeOriginV1::NominalApplication(_)) {
            let key = source
                .metadata()
                .identities
                .canonical_key::<_, scoop_identity::ExactTypeKey>(record.exact())
                .map_err(hir::SharedTypeMetadataError::from)?;
            super::applications::validate(&mut comparison, dependencies, key.as_ref())?;
        }
    }
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        if builtin.declaration_key().origin() == source.provider() {
            comparison.builtin(builtin)?;
        }
    }
    super::helpers::validate(&mut comparison, core, shapes)?;
    comparison.expected.sort_unstable();
    let source_count = comparison.expected.len();
    for record in types.records() {
        if comparison.expected[..source_count]
            .binary_search(&record.exact())
            .is_ok()
        {
            continue;
        }
        if let mir::MirTypeOriginV1::GeneratedNominal { role, .. } = record.origin() {
            super::helpers::generated(&mut comparison, record, role)?;
        }
    }
    comparison.finish()
}

pub(super) struct Comparison<'s, 'g> {
    pub(super) source: hir::CheckedSharedTypeFoundationV1<'s>,
    pub(super) inheritance: &'g hir::CheckedNominalInheritanceGraphV1<'g>,
    pub(super) types: &'s mir::CanonicalParamFreeMirTypeExportsV1,
    pub(super) payload_types: &'s dyn mir::MirTypeBridgeTypeLookupV1,

    expected: Vec<PersistentExactTypeId>,
}

impl<'s> Comparison<'s, '_> {
    pub(super) fn exact(
        &mut self,
        owner: PersistentTypeId,
    ) -> Result<PersistentExactTypeId, Error> {
        Ok(self
            .source
            .metadata()
            .signature_exact_type(&SignatureTypeKey::Nominal(owner))?)
    }

    pub(super) fn require_type(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<&'s mir::ParamFreeMirTypeExportV1, Error> {
        let record = self.types.get(exact).ok_or(Error::MissingType(exact))?;
        scoop_wire::allocation::try_reserve(&mut self.expected, 1, &WirePath::root())?;
        self.expected.push(exact);
        Ok(record)
    }

    pub(super) fn facts(
        &mut self,
        source: PersistentExactTypeId,
        record: &mir::ParamFreeMirTypeExportV1,
    ) -> Result<(), Error> {
        let facts = self
            .source
            .facts()
            .get_checked(source)
            .ok_or(Error::MissingFacts(source))?
            .record();
        let kind = match facts.kind() {
            hir::ExactTypeKindV1::Value {
                zst: hir::ZstStatus::ZeroSized,
            } => mir::MirValueKindV1::ZeroSizedValue,
            hir::ExactTypeKindV1::Value {
                zst: hir::ZstStatus::NonZero,
            } => mir::MirValueKindV1::NonZeroValue,
            hir::ExactTypeKindV1::Reference => mir::MirValueKindV1::Reference,
        };
        Error::require(
            record.exact(),
            Component::Facts,
            record.facts().kind() == kind && record.facts().gc() == gc(facts.gc()),
        )
    }

    pub(super) fn inheritance(
        &mut self,
        source: PersistentExactTypeId,
        record: &mir::ParamFreeMirTypeExportV1,
    ) -> Result<(), Error> {
        let node = self
            .inheritance
            .get(source)
            .ok_or(Error::MissingInheritance(source))?;
        let expected = node.edges();
        let base = match expected.direct_base() {
            hir::DirectClassBaseV1::NoClassBase => mir::MirBaseClassV1::None,
            hir::DirectClassBaseV1::ClassBase { exact } => mir::MirBaseClassV1::Base(exact),
        };
        Error::require(
            record.exact(),
            Component::Base,
            record.base_and_interfaces().base == base,
        )?;

        Error::require(
            record.exact(),
            Component::Interfaces,
            record.base_and_interfaces().interfaces == expected.direct_interfaces(),
        )
    }

    fn builtin(&mut self, builtin: CoreBuiltinNominal) -> Result<(), Error> {
        let nominal = builtin.identity_record().id();
        let exact = self.exact(nominal)?;
        let record = self.require_type(exact)?;
        Error::require(
            exact,
            Component::Origin,
            record.origin() == &mir::MirTypeOriginV1::SourceNominal(nominal),
        )?;
        self.facts(exact, record)?;
        let matches = match (builtin, record.representation()) {
            (
                CoreBuiltinNominal::Unit,
                mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::Unit),
            ) => true,
            (
                CoreBuiltinNominal::Any,
                mir::MirTypeRepresentationV1::Class {
                    release_policy: mir::MirClassReleasePolicyV1::None,
                    kind: mir::MirClassKindV1::Abstract,
                    declared_fields,
                },
            ) => declared_fields.is_empty(),
            _ => false,
        };
        Error::require(exact, Component::Representation, matches)?;
        Error::require(
            exact,
            Component::Base,
            record.base_and_interfaces().base == mir::MirBaseClassV1::None,
        )?;
        Error::require(
            exact,
            Component::Interfaces,
            record.base_and_interfaces().interfaces.is_empty(),
        )
    }

    fn finish(mut self) -> Result<(), Error> {
        self.expected.sort_unstable();
        self.expected.dedup();

        let mut expected = self.expected.into_iter();
        for record in self.types.records() {
            if expected.next() != Some(record.exact()) {
                return Err(Error::UnexpectedType(record.exact()));
            }
        }
        if let Some(exact) = expected.next() {
            return Err(Error::MissingType(exact));
        }
        Ok(())
    }
}

pub(super) const fn gc(source: hir::ExactTypeGcV1) -> mir::MirGcKindV1 {
    match source {
        hir::ExactTypeGcV1::GcFree => mir::MirGcKindV1::GcFree,
        hir::ExactTypeGcV1::ContainsManagedReferences => {
            mir::MirGcKindV1::ContainsManagedReferences
        }
    }
}
