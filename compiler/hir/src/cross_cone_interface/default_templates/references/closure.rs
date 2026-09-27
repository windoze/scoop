use std::cmp::Ordering;
use std::fmt;

use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};

use super::{
    ExportDefaultCallableTargetV1, ExportDefaultReferenceKindV1, ExportDefaultReferenceSetV1,
    ExportDefaultReferenceV1,
};
use crate::{
    DefaultBodyProviderTypeSiteV1, DefaultConstructorRefV1, DefaultFieldRefV1,
    ExportDefaultTemplateV1, ExportDefinitionSourceV1,
};

mod compare;
mod observer;
mod visitor;
mod walk;

pub use visitor::*;

use compare::{CallableTargetView, ConstructorTargetView, FieldTargetView};
use observer::ClosureObserver;

impl ExportDefaultTemplateV1 {
    /// Checks that the reference index exactly matches the typed body bindings.
    pub fn validate_reference_closure(
        &self,
        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let mut observer = ClosureObserver::new(self.references(), path)?;
        self.body().visit_direct_references(
            self.locals(),
            self.definition_origin(),
            &mut observer,
            path,
        )?;
        observer.finish()
    }
}

struct ReferenceDomain<'a, T> {
    records: &'a [ExportDefaultReferenceV1<T>],
    seen: Vec<bool>,
}

impl<'a, T> ReferenceDomain<'a, T> {
    fn new(records: &'a [ExportDefaultReferenceV1<T>], path: &WirePath) -> Result<Self, WireError> {
        let mut seen = Vec::new();
        scoop_wire::allocation::try_reserve(&mut seen, records.len(), path)?;
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

        path: &WirePath,
    ) -> Result<Self, WireError> {
        Ok(Self {
            callables: ReferenceDomain::new(references.callables(), path)?,
            constructors: ReferenceDomain::new(references.constructors(), path)?,
            types: ReferenceDomain::new(references.types(), path)?,
            globals: ReferenceDomain::new(references.globals(), path)?,
            singletons: ReferenceDomain::new(references.singleton_values(), path)?,
            fields: ReferenceDomain::new(references.fields(), path)?,
        })
    }

    fn finish(&self) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        check_extras(ExportDefaultReferenceKindV1::Callable, &self.callables.seen)?;
        check_extras(
            ExportDefaultReferenceKindV1::Constructor,
            &self.constructors.seen,
        )?;
        check_extras(ExportDefaultReferenceKindV1::Type, &self.types.seen)?;
        check_extras(ExportDefaultReferenceKindV1::Global, &self.globals.seen)?;
        check_extras(
            ExportDefaultReferenceKindV1::Singleton,
            &self.singletons.seen,
        )?;
        check_extras(ExportDefaultReferenceKindV1::Field, &self.fields.seen)
    }
}

#[allow(clippy::too_many_arguments)]
fn observe_record<T>(
    records: &[ExportDefaultReferenceV1<T>],
    seen: &mut [bool],
    origin: &ExportDefinitionSourceV1,
    kind: ExportDefaultReferenceKindV1,
    site: ExportDefaultReferenceOccurrenceSiteV1,
    mut compare_target: impl FnMut(&T, &WirePath) -> Result<Ordering, WireError>,

    path: &WirePath,
) -> Result<(), ExportDefaultReferenceClosureValidationError> {
    let mut start = 0;
    let mut end = records.len();
    while start < end {
        let middle = start + (end - start) / 2;
        let record = &records[middle];
        let ordering = compare_target(record.target(), path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?
            .then_with(|| record.definition_origin().cmp(origin));
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
) -> Result<(), ExportDefaultReferenceClosureValidationError> {
    for (index, seen) in seen.iter().copied().enumerate() {
        if !seen {
            return Err(ExportDefaultReferenceClosureValidationError::Extra { kind, index });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceOccurrenceSiteV1 {
    CallableBodyHeader { field: u8, index: usize },
    InitializationHeader,
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
