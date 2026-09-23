use std::cmp::Ordering;
use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{
    ExportDefaultAccessWitnessV1, ExportDefaultCallDomainV1, ExportDefaultCallableTargetV1,
    ExportDefaultReferenceKindV1, ExportDefaultReferenceSetV1, ExportDefaultReferenceV1,
};
use crate::{
    CallableInterfaceRecordV1, DefaultBodyProviderTypeSiteV1, DefaultConstructorRefV1,
    DefaultFieldRefV1, ExportDefaultTemplateV1, ExportDefinitionSourceV1, PublicLookupAccessV1,
};

mod compare;
mod observer;
mod visitor;
mod walk;

pub use visitor::*;

use compare::{CallableTargetView, ConstructorTargetView, FieldTargetView};
use observer::ClosureObserver;

impl ExportDefaultTemplateV1 {
    /// Proves that the declared six-domain reference set is exactly the
    /// canonical deduplicated closure of direct bindings in `locals` and
    /// `body`.
    ///
    /// This pass assumes target and origin authorities are checked by the
    /// reference-envelope pass. It still derives the witness from the owner
    /// interface and compares the complete `(target, origin, witness)` record.
    pub fn validate_reference_closure_semantics(
        &self,
        owner_interface: &CallableInterfaceRecordV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let expected_owner = self.key().owner();
        let actual_owner = owner_interface.declaration();
        if actual_owner != expected_owner {
            return Err(
                ExportDefaultReferenceClosureValidationError::OwnerInterface {
                    expected: expected_owner,
                    actual: actual_owner,
                },
            );
        }

        let witness = ExportDefaultAccessWitnessV1::new(
            expected_owner,
            call_domain(owner_interface.access()),
        );
        self.validate_reference_closure(WitnessExpectation::Public(witness), meter, path)
    }

    /// Checks the exact direct reference closure of any shared source default.
    /// Target access and publisher call domains must also be replayed from the
    /// actual provider declarations by the artifact reader.
    pub fn validate_source_reference_closure(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.validate_reference_closure(
            WitnessExpectation::SourceOwner(self.key().owner()),
            meter,
            path,
        )
    }

    fn validate_reference_closure(
        &self,
        witness: WitnessExpectation,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let mut observer = ClosureObserver::new(self.references(), witness, meter, path)?;
        self.body().visit_direct_references(
            self.locals(),
            self.definition_origin(),
            &mut observer,
            meter,
            path,
        )?;
        observer.finish(meter, path)
    }
}

enum WitnessExpectation {
    Public(ExportDefaultAccessWitnessV1),
    SourceOwner(CallableTemplateOrigin),
}

impl WitnessExpectation {
    fn compare(
        &self,
        actual: &ExportDefaultAccessWitnessV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Ordering, WireError> {
        match self {
            Self::Public(expected) => {
                actual.charge_comparison(expected, meter, path)?;
                Ok(actual.cmp(expected))
            }
            Self::SourceOwner(expected) => {
                meter.charge_work(1, path)?;
                Ok(actual.owner().cmp(expected))
            }
        }
    }
}

struct ReferenceDomain<'a, T> {
    records: &'a [ExportDefaultReferenceV1<T>],
    seen: Vec<bool>,
}

impl<'a, T> ReferenceDomain<'a, T> {
    fn new(
        records: &'a [ExportDefaultReferenceV1<T>],
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut seen = Vec::new();
        meter.try_reserve_collection_slots(&mut seen, records.len(), path)?;
        seen.resize(records.len(), false);
        Ok(Self { records, seen })
    }
}

struct ReferenceDomains<'a> {
    callables: ReferenceDomain<'a, ExportDefaultCallableTargetV1>,
    constructors: ReferenceDomain<'a, DefaultConstructorRefV1>,
    types: ReferenceDomain<'a, SignatureTypeKey>,
    globals: ReferenceDomain<'a, PersistentPropertyId>,
    singletons: ReferenceDomain<'a, PersistentObjectValueId>,
    fields: ReferenceDomain<'a, DefaultFieldRefV1>,
}

impl<'a> ReferenceDomains<'a> {
    fn new(
        references: &'a ExportDefaultReferenceSetV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        Ok(Self {
            callables: ReferenceDomain::new(references.callables(), meter, path)?,
            constructors: ReferenceDomain::new(references.constructors(), meter, path)?,
            types: ReferenceDomain::new(references.types(), meter, path)?,
            globals: ReferenceDomain::new(references.globals(), meter, path)?,
            singletons: ReferenceDomain::new(references.singleton_values(), meter, path)?,
            fields: ReferenceDomain::new(references.fields(), meter, path)?,
        })
    }

    fn finish(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        check_extras(
            ExportDefaultReferenceKindV1::Callable,
            &self.callables.seen,
            meter,
            path,
        )?;
        check_extras(
            ExportDefaultReferenceKindV1::Constructor,
            &self.constructors.seen,
            meter,
            path,
        )?;
        check_extras(
            ExportDefaultReferenceKindV1::Type,
            &self.types.seen,
            meter,
            path,
        )?;
        check_extras(
            ExportDefaultReferenceKindV1::Global,
            &self.globals.seen,
            meter,
            path,
        )?;
        check_extras(
            ExportDefaultReferenceKindV1::Singleton,
            &self.singletons.seen,
            meter,
            path,
        )?;
        check_extras(
            ExportDefaultReferenceKindV1::Field,
            &self.fields.seen,
            meter,
            path,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn observe_record<T>(
    records: &[ExportDefaultReferenceV1<T>],
    seen: &mut [bool],
    origin: &ExportDefinitionSourceV1,
    witness: &WitnessExpectation,
    kind: ExportDefaultReferenceKindV1,
    site: ExportDefaultReferenceOccurrenceSiteV1,
    mut compare_target: impl FnMut(&T, &mut BudgetMeter, &WirePath) -> Result<Ordering, WireError>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExportDefaultReferenceClosureValidationError> {
    let mut start = 0;
    let mut end = records.len();
    while start < end {
        meter
            .charge_work(1, path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        let middle = start + (end - start) / 2;
        let record = &records[middle];
        let witness_ordering = witness.compare(record.witness(), meter, path)?;
        let ordering = compare_target(record.target(), meter, path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?
            .then_with(|| record.definition_origin().cmp(origin))
            .then(witness_ordering);
        match ordering {
            Ordering::Less => start = middle + 1,
            Ordering::Greater => end = middle,
            Ordering::Equal => {
                seen[middle] = true;
                return Ok(());
            }
        }
    }
    Err(ExportDefaultReferenceClosureValidationError::Missing {
        kind,
        site,
        insertion_index: start,
        definition_origin: Box::new(origin.clone()),
    })
}

fn check_extras(
    kind: ExportDefaultReferenceKindV1,
    seen: &[bool],
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExportDefaultReferenceClosureValidationError> {
    for (index, seen) in seen.iter().copied().enumerate() {
        meter
            .charge_work(1, path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        if !seen {
            return Err(ExportDefaultReferenceClosureValidationError::Extra { kind, index });
        }
    }
    Ok(())
}

const fn call_domain(access: PublicLookupAccessV1) -> ExportDefaultCallDomainV1 {
    match access {
        PublicLookupAccessV1::DirectOnly => ExportDefaultCallDomainV1::DirectPublic,
        PublicLookupAccessV1::PublicSlot => ExportDefaultCallDomainV1::DirectAndPublicSlot,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceOccurrenceSiteV1 {
    TemplateLocalType { index: usize },
    BodyType(DefaultBodyProviderTypeSiteV1),
    Expression,
    Statement,
    Pattern,
    Assignment,
    BindingAction,
    IteratorProtocol,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceClosureValidationError {
    OwnerInterface {
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    Missing {
        kind: ExportDefaultReferenceKindV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
        insertion_index: usize,
        definition_origin: Box<ExportDefinitionSourceV1>,
    },
    Extra {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
    },
    Resource(WireError),
}

impl fmt::Display for ExportDefaultReferenceClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerInterface { expected, actual } => write!(
                formatter,
                "default-template owner {expected:?} does not match reference-closure owner interface {actual:?}"
            ),
            Self::Missing {
                kind,
                site,
                insertion_index,
                ..
            } => write!(
                formatter,
                "default body {site:?} requires a missing {kind} reference at canonical insertion index {insertion_index}"
            ),
            Self::Extra { kind, index } => write!(
                formatter,
                "default reference closure contains an unused {kind} reference at index {index}"
            ),
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default reference closure resource failure: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ExportDefaultReferenceClosureValidationError {}

impl From<WireError> for ExportDefaultReferenceClosureValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

#[cfg(test)]
mod tests;
