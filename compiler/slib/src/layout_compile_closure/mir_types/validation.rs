use scoop_hir as hir;
use scoop_identity::{
    CoreBuiltinNominal, PersistentExactTypeId, PersistentTypeId, SignatureTypeKey,
};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

use super::{Error, SharedMirTypeComponent as Component};

/// Replays type and finite-shape constituents from checked shared declarations.
/// This does not validate callables, dispatch, selected uses or machine layouts.
pub fn validate_shared_mir_type_exports(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    inheritance: &hir::CheckedNominalInheritanceGraphV1<'_>,
    core: &hir::CoreBootstrapInterfaceSectionV1,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    shapes: &mir::CanonicalMirShapeSupportsV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut comparison = Comparison {
        source,
        inheritance,
        types,
        meter,
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
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        if builtin.declaration_key().origin() == source.provider() {
            comparison.builtin(builtin)?;
        }
    }
    super::helpers::validate(&mut comparison, core, shapes)?;
    comparison.finish()
}

pub(super) fn validate(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    inheritance: &hir::CheckedNominalInheritanceGraphV1<'_>,
    core: &hir::CoreBootstrapInterfaceSectionV1,
    mir: &mir::TypeResolvedCrossConeMirTypeBridgeSectionV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    validate_shared_mir_type_exports(
        source,
        inheritance,
        core,
        mir.types(),
        mir.shape_support(),
        meter,
    )
}

pub(super) struct Comparison<'s, 'g, 'm> {
    pub(super) source: hir::CheckedSharedTypeFoundationV1<'s>,
    pub(super) inheritance: &'g hir::CheckedNominalInheritanceGraphV1<'g>,
    pub(super) types: &'s mir::CanonicalParamFreeMirTypeExportsV1,
    pub(super) meter: &'m mut BudgetMeter,
    expected: Vec<PersistentExactTypeId>,
}

impl<'s> Comparison<'s, '_, '_> {
    pub(super) fn work(&mut self, count: usize) -> Result<(), Error> {
        Ok(self.meter.charge_work(count as u64, &WirePath::root())?)
    }

    pub(super) fn exact(
        &mut self,
        owner: PersistentTypeId,
    ) -> Result<PersistentExactTypeId, Error> {
        Ok(self
            .source
            .metadata()
            .signature_exact_type(&SignatureTypeKey::Nominal(owner), self.meter)?)
    }

    pub(super) fn require_type(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<&'s mir::ParamFreeMirTypeExportV1, Error> {
        self.work(self.types.records().len().checked_ilog2().unwrap_or(0) as usize + 1)?;
        let record = self.types.get(exact).ok_or(Error::MissingType(exact))?;
        self.meter
            .try_reserve_collection_slots(&mut self.expected, 1, &WirePath::root())?;
        self.expected.push(exact);
        Ok(record)
    }

    pub(super) fn facts(
        &mut self,
        source: PersistentExactTypeId,
        record: &mir::ParamFreeMirTypeExportV1,
    ) -> Result<(), Error> {
        self.work(
            self.source
                .facts()
                .records()
                .len()
                .checked_ilog2()
                .unwrap_or(0) as usize
                + 1,
        )?;
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
        self.work(self.inheritance.node_count().checked_ilog2().unwrap_or(0) as usize + 1)?;
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
        self.work(expected.direct_interfaces().len())?;
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
        self.work(
            self.expected
                .len()
                .saturating_mul(self.expected.len().checked_ilog2().unwrap_or(0) as usize + 1),
        )?;
        self.expected.sort_unstable();
        self.expected.dedup();
        self.work(self.types.records().len())?;
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
