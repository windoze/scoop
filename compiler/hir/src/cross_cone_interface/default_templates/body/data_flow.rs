use std::fmt;

use scoop_identity::{LocalValueSelector, PersistentFieldId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use crate::{
    CanonicalBooleanV1, DefaultBindingTemporaryV1, ExportDefaultTemplateV1, TemplateLocalRecordV1,
};

mod binding;
mod control;
mod expression;

#[cfg(test)]
mod tests;

/// Resolves the one foundation fact that cannot be reconstructed from the
/// portable binding shape itself.
pub trait DefaultLocalDataFlowSemanticAuthority<E> {
    fn default_binding_struct_field_index(
        &mut self,
        template: &ExportDefaultTemplateV1,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
    ) -> Result<u32, E>;
}

impl ExportDefaultTemplateV1 {
    /// Proves definite local definition, mutability, loop nesting, and the
    /// exact binding-shape/action schedule for this template.
    ///
    /// Provider type/origin envelopes must already be valid. Operation
    /// typing and nested-callable ABI validation remain independent passes.
    pub fn validate_local_data_flow_semantics<A, E>(
        &self,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>>
    where
        A: DefaultLocalDataFlowSemanticAuthority<E>,
    {
        Validator::new(self, authority, meter, path)?.run()
    }
}

struct Validator<'a, A, E> {
    template: &'a ExportDefaultTemplateV1,
    authority: &'a mut A,
    owners: Vec<Option<DefinitionOwner>>,
    next_plan: u32,
    meter: &'a mut BudgetMeter,
    path: &'a WirePath,
    error: std::marker::PhantomData<fn() -> E>,
}

impl<'a, A, E> Validator<'a, A, E>
where
    A: DefaultLocalDataFlowSemanticAuthority<E>,
{
    fn new(
        template: &'a ExportDefaultTemplateV1,
        authority: &'a mut A,
        meter: &'a mut BudgetMeter,
        path: &'a WirePath,
    ) -> Result<Self, ExportDefaultLocalDataFlowValidationError<E>> {
        let mut owners = Vec::new();
        meter
            .try_reserve_collection_slots(&mut owners, template.locals().records().len(), path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        owners.resize(template.locals().records().len(), None);
        Ok(Self {
            template,
            authority,
            owners,
            next_plan: 0,
            meter,
            path,
            error: std::marker::PhantomData,
        })
    }

    fn run(mut self) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        self.enter_node(1)?;
        let mut available = self.empty_bits()?;

        if let Some(receiver) = self.template.receiver().receiver() {
            self.define_local(
                receiver.local(),
                Some(receiver.value_type()),
                Some(CanonicalBooleanV1::False),
                DefaultLocalDataFlowSiteV1::Receiver,
                &mut available,
                DefinitionOwner::Ordinary,
            )?;
        }
        for parameter in self.template.value_parameters().parameters() {
            self.define_local(
                parameter.local(),
                None,
                Some(CanonicalBooleanV1::False),
                DefaultLocalDataFlowSiteV1::ValueParameter {
                    position: parameter.position(),
                },
                &mut available,
                DefinitionOwner::Ordinary,
            )?;
        }

        let region = Region {
            owner: DefinitionOwner::Ordinary,
            loop_depth: 0,
            depth: 2,
        };
        let flow = self.validate_statements(
            self.template.body().statements(),
            Flow::falling_through(available),
            true,
            region,
        )?;
        self.validate_expression(
            self.template.body().value(),
            &flow.available,
            flow.falls_through,
            2,
        )
    }

    fn local_index(
        &mut self,
        selector: &LocalValueSelector,
        site: DefaultLocalDataFlowSiteV1,
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError<E>> {
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        self.template
            .locals()
            .records()
            .binary_search_by(|record| record.selector().cmp(selector))
            .map_err(|_| self.local_error(site, selector, DefaultLocalDataFlowLocalError::Missing))
    }

    fn check_local_shape(
        &mut self,
        index: usize,
        actual_type: Option<&SignatureTypeKey>,
        actual_mutability: Option<CanonicalBooleanV1>,
        site: DefaultLocalDataFlowSiteV1,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        let record = &self.template.locals().records()[index];
        if let Some(actual) = actual_type
            && record.value_type() != actual
        {
            return Err(self.local_error(
                site,
                record.selector(),
                DefaultLocalDataFlowLocalError::Type {
                    expected: Box::new(record.value_type().clone()),
                    actual: Box::new(actual.clone()),
                },
            ));
        }
        if let Some(actual) = actual_mutability
            && record.mutable() != actual
        {
            return Err(self.local_error(
                site,
                record.selector(),
                DefaultLocalDataFlowLocalError::Mutability {
                    expected: record.mutable(),
                    actual,
                },
            ));
        }
        Ok(())
    }

    fn use_local(
        &mut self,
        selector: &LocalValueSelector,
        actual_type: Option<&SignatureTypeKey>,
        site: DefaultLocalDataFlowSiteV1,
        available: &[bool],
        reachable: bool,
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError<E>> {
        let index = self.local_index(selector, site)?;
        self.check_local_shape(index, actual_type, None, site)?;
        if reachable && !available[index] {
            return Err(self.local_error(
                site,
                selector,
                DefaultLocalDataFlowLocalError::UseBeforeDefinition,
            ));
        }
        Ok(index)
    }

    #[allow(clippy::too_many_arguments)]
    fn define_local(
        &mut self,
        selector: &LocalValueSelector,
        actual_type: Option<&SignatureTypeKey>,
        actual_mutability: Option<CanonicalBooleanV1>,
        site: DefaultLocalDataFlowSiteV1,
        available: &mut [bool],
        owner: DefinitionOwner,
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError<E>> {
        let index = self.local_index(selector, site)?;
        self.check_local_shape(index, actual_type, actual_mutability, site)?;
        if available[index] {
            return Err(self.local_error(
                site,
                selector,
                DefaultLocalDataFlowLocalError::AlreadyDefined,
            ));
        }
        self.claim_owner(index, site, owner)?;
        available[index] = true;
        Ok(index)
    }

    fn claim_local(
        &mut self,
        selector: &LocalValueSelector,
        actual_type: Option<&SignatureTypeKey>,
        actual_mutability: Option<CanonicalBooleanV1>,
        site: DefaultLocalDataFlowSiteV1,
        owner: DefinitionOwner,
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError<E>> {
        let index = self.local_index(selector, site)?;
        self.check_local_shape(index, actual_type, actual_mutability, site)?;
        self.claim_owner(index, site, owner)?;
        Ok(index)
    }

    fn claim_owner(
        &mut self,
        index: usize,
        site: DefaultLocalDataFlowSiteV1,
        owner: DefinitionOwner,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        match self.owners[index] {
            None => self.owners[index] = Some(owner),
            Some(actual) if actual == owner => {}
            Some(actual) => {
                let selector = self.template.locals().records()[index].selector();
                return Err(self.local_error(
                    site,
                    selector,
                    DefaultLocalDataFlowLocalError::DefinitionOwner {
                        expected: owner.into_public(),
                        actual: actual.into_public(),
                    },
                ));
            }
        }
        Ok(())
    }

    fn require_mutable_assignment(
        &mut self,
        selector: &LocalValueSelector,
        site: DefaultLocalDataFlowSiteV1,
        available: &mut [bool],
        owner: DefinitionOwner,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        let index = self.local_index(selector, site)?;
        self.check_local_shape(index, None, Some(CanonicalBooleanV1::True), site)?;
        if !available[index] {
            self.claim_owner(index, site, owner)?;
            available[index] = true;
        }
        Ok(())
    }

    fn local_record(&self, index: usize) -> &TemplateLocalRecordV1 {
        &self.template.locals().records()[index]
    }

    fn local_error(
        &self,
        site: DefaultLocalDataFlowSiteV1,
        selector: &LocalValueSelector,
        error: DefaultLocalDataFlowLocalError,
    ) -> ExportDefaultLocalDataFlowValidationError<E> {
        ExportDefaultLocalDataFlowValidationError::Local {
            site,
            selector: Box::new(selector.clone()),
            error,
        }
    }

    fn empty_bits(&mut self) -> Result<Vec<bool>, ExportDefaultLocalDataFlowValidationError<E>> {
        let mut bits = Vec::new();
        self.meter
            .try_reserve_collection_slots(
                &mut bits,
                self.template.locals().records().len(),
                self.path,
            )
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        bits.resize(self.template.locals().records().len(), false);
        Ok(bits)
    }

    fn copy_bits(
        &mut self,
        source: &[bool],
    ) -> Result<Vec<bool>, ExportDefaultLocalDataFlowValidationError<E>> {
        let mut copy = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut copy, source.len(), self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        self.meter
            .charge_work(source.len() as u64, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        copy.extend_from_slice(source);
        Ok(copy)
    }

    fn abrupt_flow(
        &mut self,
        available: Vec<bool>,
        outcome: DefaultLoopControlV1,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError<E>> {
        let state = self.copy_bits(&available)?;
        Ok(Flow {
            available,
            falls_through: false,
            abrupt: AbruptOutcomes::single(outcome, state),
        })
    }

    fn merge_abrupt_outcomes(
        &mut self,
        target: &mut AbruptOutcomes,
        other: AbruptOutcomes,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for (outcome, state) in other.into_entries() {
            let Some(state) = state else {
                continue;
            };
            match target.slot_mut(outcome) {
                None => *target.slot_mut(outcome) = Some(state),
                Some(current) => self.intersect_bits(current, &state)?,
            }
        }
        Ok(())
    }

    fn intersect_bits(
        &mut self,
        target: &mut [bool],
        other: &[bool],
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        for (slot, other) in target.iter_mut().zip(other) {
            self.meter
                .charge_work(1, self.path)
                .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
            *slot &= *other;
        }
        Ok(())
    }

    fn next_plan_owner(
        &mut self,
    ) -> Result<DefinitionOwner, ExportDefaultLocalDataFlowValidationError<E>> {
        let plan = self.next_plan;
        self.next_plan = self.next_plan.checked_add(1).ok_or_else(|| {
            ExportDefaultLocalDataFlowValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                self.path.clone(),
                None,
            ))
        })?;
        Ok(DefinitionOwner::ForPlan(plan))
    }

    fn enter_node(
        &mut self,
        depth: u64,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        self.meter
            .charge_nodes(1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)
    }

    fn enter_edge(
        &mut self,
        child_depth: u64,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError<E>> {
        self.meter
            .check_semantic_depth(child_depth, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        self.meter
            .charge_edges(1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)
    }

    fn child_depth(&self, depth: u64) -> Result<u64, ExportDefaultLocalDataFlowValidationError<E>> {
        depth.checked_add(1).ok_or_else(|| {
            ExportDefaultLocalDataFlowValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                self.path.clone(),
                None,
            ))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Region {
    owner: DefinitionOwner,
    loop_depth: u64,
    depth: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DefinitionOwner {
    Ordinary,
    ForPlan(u32),
}

impl DefinitionOwner {
    const fn into_public(self) -> DefaultLocalDefinitionOwnerV1 {
        match self {
            Self::Ordinary => DefaultLocalDefinitionOwnerV1::Ordinary,
            Self::ForPlan(index) => DefaultLocalDefinitionOwnerV1::ForPlan { index },
        }
    }
}

struct Flow {
    available: Vec<bool>,
    falls_through: bool,
    abrupt: AbruptOutcomes,
}

impl Flow {
    fn falling_through(available: Vec<bool>) -> Self {
        Self {
            available,
            falls_through: true,
            abrupt: AbruptOutcomes::default(),
        }
    }
}

#[derive(Default)]
struct AbruptOutcomes {
    returns: Option<Vec<bool>>,
    throws: Option<Vec<bool>>,
    breaks: Option<Vec<bool>>,
    continues: Option<Vec<bool>>,
}

impl AbruptOutcomes {
    fn single(outcome: DefaultLoopControlV1, state: Vec<bool>) -> Self {
        let mut outcomes = Self::default();
        *outcomes.slot_mut(outcome) = Some(state);
        outcomes
    }

    fn slot_mut(&mut self, outcome: DefaultLoopControlV1) -> &mut Option<Vec<bool>> {
        match outcome {
            DefaultLoopControlV1::Return => &mut self.returns,
            DefaultLoopControlV1::Throw => &mut self.throws,
            DefaultLoopControlV1::Break => &mut self.breaks,
            DefaultLoopControlV1::Continue => &mut self.continues,
        }
    }

    fn take(&mut self, outcome: DefaultLoopControlV1) -> Option<Vec<bool>> {
        self.slot_mut(outcome).take()
    }

    fn states(&self) -> impl Iterator<Item = &[bool]> {
        [
            self.returns.as_deref(),
            self.throws.as_deref(),
            self.breaks.as_deref(),
            self.continues.as_deref(),
        ]
        .into_iter()
        .flatten()
    }

    fn into_entries(self) -> [(DefaultLoopControlV1, Option<Vec<bool>>); 4] {
        [
            (DefaultLoopControlV1::Return, self.returns),
            (DefaultLoopControlV1::Throw, self.throws),
            (DefaultLoopControlV1::Break, self.breaks),
            (DefaultLoopControlV1::Continue, self.continues),
        ]
    }

    const fn is_empty(&self) -> bool {
        self.returns.is_none()
            && self.throws.is_none()
            && self.breaks.is_none()
            && self.continues.is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultLocalDataFlowSiteV1 {
    Receiver,
    ValueParameter {
        position: u32,
    },
    Expression,
    AddressOf,
    Assignment,
    Capture {
        index: usize,
    },
    PatternBinding,
    Catch,
    ForTemporary(DefaultForTemporaryRoleV1),
    BindingSubject,
    BindingAction {
        index: usize,
        role: DefaultBindingActionLocalRoleV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultForTemporaryRoleV1 {
    Source,
    ConformanceSource,
    Iterator,
    NextResult,
    NextElement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBindingActionLocalRoleV1 {
    Source,
    Result,
    Target,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultLocalDefinitionOwnerV1 {
    Ordinary,
    ForPlan { index: u32 },
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultLocalDataFlowLocalError {
    Missing,
    UseBeforeDefinition,
    AlreadyDefined,
    Type {
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    Mutability {
        expected: CanonicalBooleanV1,
        actual: CanonicalBooleanV1,
    },
    DefinitionOwner {
        expected: DefaultLocalDefinitionOwnerV1,
        actual: DefaultLocalDefinitionOwnerV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultLoopControlV1 {
    Return,
    Throw,
    Break,
    Continue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBindingShapeActionKindV1 {
    Bind,
    TupleProjection,
    StructProjection,
    Component,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBindingShapeDataFlowValidationError<E> {
    ClassComponentOrder {
        position: usize,
        expected: u32,
        actual: u32,
    },
    StructProjection {
        action_index: usize,
        error: Box<E>,
    },
    MissingAction {
        kind: DefaultBindingShapeActionKindV1,
    },
    MultipleActions {
        kind: DefaultBindingShapeActionKindV1,
    },
    ActionKind {
        index: usize,
        expected: DefaultBindingShapeActionKindV1,
    },
    ReusedAction {
        index: usize,
    },
    ExtraAction {
        index: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultLocalDataFlowValidationError<E> {
    Local {
        site: DefaultLocalDataFlowSiteV1,
        selector: Box<LocalValueSelector>,
        error: DefaultLocalDataFlowLocalError,
    },
    LoopControlOutsideLoop {
        control: DefaultLoopControlV1,
    },
    ForTemporaryAlias {
        first: DefaultForTemporaryRoleV1,
        second: DefaultForTemporaryRoleV1,
        selector: Box<LocalValueSelector>,
    },
    BindingSubject {
        expected: Box<DefaultBindingTemporaryV1>,
        actual: Box<DefaultBindingTemporaryV1>,
    },
    BindingShape(Box<DefaultBindingShapeDataFlowValidationError<E>>),
    Resource(WireError),
}

impl fmt::Display for DefaultLocalDataFlowLocalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => formatter.write_str("is absent from the canonical local table"),
            Self::UseBeforeDefinition => formatter.write_str("is used before its definition"),
            Self::AlreadyDefined => formatter.write_str("is defined more than once on one path"),
            Self::Type { expected, actual } => {
                write!(formatter, "has type {actual:?}, expected {expected:?}")
            }
            Self::Mutability { expected, actual } => {
                write!(
                    formatter,
                    "has mutability {actual:?}, expected {expected:?}"
                )
            }
            Self::DefinitionOwner { expected, actual } => write!(
                formatter,
                "is owned by {actual:?}, but this definition belongs to {expected:?}"
            ),
        }
    }
}

impl<E: fmt::Display> fmt::Display for DefaultBindingShapeDataFlowValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClassComponentOrder {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "binding class component {position} has index {actual}, expected {expected}"
            ),
            Self::StructProjection {
                action_index,
                error,
            } => write!(
                formatter,
                "cannot resolve binding struct projection action {action_index}: {error}"
            ),
            Self::MissingAction { kind } => {
                write!(formatter, "binding shape is missing its {kind:?} action")
            }
            Self::MultipleActions { kind } => {
                write!(formatter, "binding shape matches multiple {kind:?} actions")
            }
            Self::ActionKind { index, expected } => write!(
                formatter,
                "binding action {index} does not produce the expected {expected:?} result"
            ),
            Self::ReusedAction { index } => {
                write!(formatter, "binding shape reuses action {index}")
            }
            Self::ExtraAction { index } => {
                write!(
                    formatter,
                    "binding action {index} is outside the checked shape"
                )
            }
        }
    }
}

impl<E: fmt::Display> fmt::Display for ExportDefaultLocalDataFlowValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local {
                site,
                selector,
                error,
            } => write!(formatter, "default local {selector:?} at {site:?} {error}"),
            Self::LoopControlOutsideLoop { control } => {
                write!(
                    formatter,
                    "default {control:?} appears outside an active loop"
                )
            }
            Self::ForTemporaryAlias {
                first,
                second,
                selector,
            } => write!(
                formatter,
                "for temporaries {first:?} and {second:?} alias local {selector:?}"
            ),
            Self::BindingSubject { expected, actual } => write!(
                formatter,
                "for binding subject {actual:?} differs from next element {expected:?}"
            ),
            Self::BindingShape(error) => write!(formatter, "invalid binding shape: {error}"),
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default local data-flow resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultLocalDataFlowValidationError<E>
{
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultBindingShapeDataFlowValidationError<E>
{
}
