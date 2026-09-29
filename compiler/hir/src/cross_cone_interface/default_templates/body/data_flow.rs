use std::fmt;

use scoop_identity::{LocalValueSelector, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};

use crate::{CanonicalBooleanV1, ExportDefaultTemplateV1};

mod control;
mod expression;

#[cfg(test)]
mod tests;

impl ExportDefaultTemplateV1 {
    /// Checks definite local definition, mutability and loop nesting.
    ///
    /// Types and declaration references have already been resolved at the reader boundary.
    pub fn validate_local_data_flow_semantics(
        &self,

        path: &WirePath,
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError> {
        Validator {
            template: self,
            path,
        }
        .run()
    }
}

struct Validator<'a> {
    template: &'a ExportDefaultTemplateV1,

    path: &'a WirePath,
}

impl<'a> Validator<'a> {
    fn run(mut self) -> Result<(), ExportDefaultLocalDataFlowValidationError> {
        let mut available = self.empty_bits()?;

        if let Some(receiver) = self.template.receiver().receiver() {
            self.define_local(
                receiver.local(),
                Some(receiver.value_type()),
                Some(CanonicalBooleanV1::False),
                DefaultLocalDataFlowSiteV1::Receiver,
                &mut available,
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
            )?;
        }

        let region = Region { loop_depth: 0 };
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
        )
    }

    fn local_index(
        &mut self,
        selector: &LocalValueSelector,
        site: DefaultLocalDataFlowSiteV1,
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError> {
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
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError> {
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
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError> {
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
    ) -> Result<usize, ExportDefaultLocalDataFlowValidationError> {
        let index = self.local_index(selector, site)?;
        self.check_local_shape(index, actual_type, actual_mutability, site)?;
        if available[index] {
            return Err(self.local_error(
                site,
                selector,
                DefaultLocalDataFlowLocalError::AlreadyDefined,
            ));
        }
        available[index] = true;
        Ok(index)
    }

    fn require_mutable_assignment(
        &mut self,
        selector: &LocalValueSelector,
        site: DefaultLocalDataFlowSiteV1,
        available: &mut [bool],
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError> {
        let index = self.local_index(selector, site)?;
        self.check_local_shape(index, None, Some(CanonicalBooleanV1::True), site)?;
        if !available[index] {
            available[index] = true;
        }
        Ok(())
    }

    fn local_error(
        &self,
        site: DefaultLocalDataFlowSiteV1,
        selector: &LocalValueSelector,
        error: DefaultLocalDataFlowLocalError,
    ) -> ExportDefaultLocalDataFlowValidationError {
        ExportDefaultLocalDataFlowValidationError::Local {
            site,
            selector: Box::new(selector.clone()),
            error,
        }
    }

    fn empty_bits(&mut self) -> Result<Vec<bool>, ExportDefaultLocalDataFlowValidationError> {
        let mut bits = Vec::new();
        scoop_wire::allocation::try_reserve(
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
    ) -> Result<Vec<bool>, ExportDefaultLocalDataFlowValidationError> {
        let mut copy = Vec::new();
        scoop_wire::allocation::try_reserve(&mut copy, source.len(), self.path)
            .map_err(ExportDefaultLocalDataFlowValidationError::Resource)?;

        copy.extend_from_slice(source);
        Ok(copy)
    }

    fn abrupt_flow(
        &mut self,
        available: Vec<bool>,
        outcome: DefaultLoopControlV1,
    ) -> Result<Flow, ExportDefaultLocalDataFlowValidationError> {
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
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError> {
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
    ) -> Result<(), ExportDefaultLocalDataFlowValidationError> {
        for (slot, other) in target.iter_mut().zip(other) {
            *slot &= *other;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Region {
    loop_depth: u64,
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
    ValueParameter { position: u32 },
    Expression,
    AddressOf,
    Assignment,
    Capture { index: usize },
    PatternBinding,
    Catch,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultLoopControlV1 {
    Return,
    Throw,
    Break,
    Continue,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultLocalDataFlowValidationError {
    UnboundCapture(u32),
    Local {
        site: DefaultLocalDataFlowSiteV1,
        selector: Box<LocalValueSelector>,
        error: DefaultLocalDataFlowLocalError,
    },
    LoopControlOutsideLoop {
        control: DefaultLoopControlV1,
    },
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
        }
    }
}

impl fmt::Display for ExportDefaultLocalDataFlowValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundCapture(index) => {
                write!(formatter, "default root has no closure input {index}")
            }
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
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default local data-flow resource failure: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ExportDefaultLocalDataFlowValidationError {}
