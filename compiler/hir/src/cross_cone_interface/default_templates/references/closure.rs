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
    TemplateLocalDefinitionV1,
};

mod compare;
mod walk;

use compare::{
    CallableTargetView, ConstructorTargetView, FieldTargetView, callable_target,
    constructor_target, field_target, signature_type,
};

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
        Validator::new(self, witness, meter, path)?.run()
    }
}

struct Validator<'a> {
    template: &'a ExportDefaultTemplateV1,
    witness: ExportDefaultAccessWitnessV1,
    domains: ReferenceDomains<'a>,
    meter: &'a mut BudgetMeter,
    path: &'a WirePath,
}

impl<'a> Validator<'a> {
    fn new(
        template: &'a ExportDefaultTemplateV1,
        witness: ExportDefaultAccessWitnessV1,
        meter: &'a mut BudgetMeter,
        path: &'a WirePath,
    ) -> Result<Self, ExportDefaultReferenceClosureValidationError> {
        let domains = ReferenceDomains::new(template.references(), meter, path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        Ok(Self {
            template,
            witness,
            domains,
            meter,
            path,
        })
    }

    fn run(mut self) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let template = self.template;
        for (index, local) in template.locals().records().iter().enumerate() {
            self.enter_node(1)?;
            let origin = match local.definition() {
                TemplateLocalDefinitionV1::Source(origin) => origin,
                TemplateLocalDefinitionV1::Synthetic => template.definition_origin(),
            };
            self.enter_leaf()?;
            self.match_type(
                local.value_type(),
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::TemplateLocalType { index },
            )?;
        }
        self.walk_body(template.body())?;
        self.domains.finish(self.meter, self.path)
    }

    fn observe_callable(
        &mut self,
        target: CallableTargetView<'_>,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.callables.records,
            &mut self.domains.callables.seen,
            origin,
            self.witness,
            ExportDefaultReferenceKindV1::Callable,
            site,
            |declared, meter, path| callable_target(declared, target, meter, path),
            self.meter,
            self.path,
        )
    }

    fn observe_constructor(
        &mut self,
        target: ConstructorTargetView<'_>,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.constructors.records,
            &mut self.domains.constructors.seen,
            origin,
            self.witness,
            ExportDefaultReferenceKindV1::Constructor,
            site,
            |declared, meter, path| constructor_target(declared, target, meter, path),
            self.meter,
            self.path,
        )
    }

    fn match_type(
        &mut self,
        target: &SignatureTypeKey,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        if matches!(target, SignatureTypeKey::Binder { .. }) {
            return Ok(());
        }
        observe_record(
            self.domains.types.records,
            &mut self.domains.types.seen,
            origin,
            self.witness,
            ExportDefaultReferenceKindV1::Type,
            site,
            |declared, meter, path| signature_type(declared, target, meter, path),
            self.meter,
            self.path,
        )
    }

    fn observe_global(
        &mut self,
        target: PersistentPropertyId,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.globals.records,
            &mut self.domains.globals.seen,
            origin,
            self.witness,
            ExportDefaultReferenceKindV1::Global,
            site,
            |declared, _, _| Ok(declared.cmp(&target)),
            self.meter,
            self.path,
        )
    }

    fn observe_singleton(
        &mut self,
        target: PersistentObjectValueId,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.singletons.records,
            &mut self.domains.singletons.seen,
            origin,
            self.witness,
            ExportDefaultReferenceKindV1::Singleton,
            site,
            |declared, _, _| Ok(declared.cmp(&target)),
            self.meter,
            self.path,
        )
    }

    fn observe_field(
        &mut self,
        target: FieldTargetView<'_>,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.fields.records,
            &mut self.domains.fields.seen,
            origin,
            self.witness,
            ExportDefaultReferenceKindV1::Field,
            site,
            |declared, meter, path| field_target(declared, target, meter, path),
            self.meter,
            self.path,
        )
    }

    fn enter_leaf(&mut self) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.meter
            .check_semantic_depth(1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.meter
            .charge_edges(1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.enter_node(1)
    }

    fn enter_scheduled_leaf(&mut self) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.enter_node(1)
    }

    fn enter_node(
        &mut self,
        depth: u64,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.meter
            .charge_nodes(1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)
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
    witness: ExportDefaultAccessWitnessV1,
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
        let ordering = compare_target(record.target(), meter, path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?
            .then_with(|| record.definition_origin().cmp(origin))
            .then_with(|| record.witness().cmp(&witness));
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

#[cfg(test)]
mod tests;
