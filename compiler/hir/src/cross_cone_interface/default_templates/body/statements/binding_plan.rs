use std::fmt;
use std::num::NonZeroU32;

use scoop_identity::SignatureTypeKey;

use super::DefaultStatementV1;
use crate::{
    DefaultBindingProjectionV1, DefaultBindingShapeV1, DefaultBindingTemporaryV1,
    DefaultCallableRefV1, DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefV1,
    DefaultExpressionV1, ExportDefinitionSourceV1,
};

mod decoded;
mod indexed;

pub use decoded::{
    DecodedDefaultAppliedOptionV1, DecodedDefaultBindingActionV1, DecodedDefaultBindingPlanV1,
    DecodedDefaultForIterationPlanV1, DecodedDefaultIteratorConformanceV1,
    DecodedDefaultIteratorNextV1, DefaultForIterationPlanResolutionError,
};

pub use indexed::{
    DefaultForIterationPlanIndexError, IndexedDefaultAppliedOptionV1,
    IndexedDefaultBindingActionV1, IndexedDefaultBindingPlanV1, IndexedDefaultForIterationPlanV1,
    IndexedDefaultIteratorConformanceV1, IndexedDefaultIteratorNextV1,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBindingActionV1(DefaultBindingActionKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum DefaultBindingActionKindV1 {
    Project {
        source: DefaultBindingTemporaryV1,
        result: DefaultBindingTemporaryV1,
        projection: DefaultBindingProjectionV1,
        definition_origin: ExportDefinitionSourceV1,
    },
    Component {
        source: DefaultBindingTemporaryV1,
        index: NonZeroU32,
        result: DefaultBindingTemporaryV1,
        setup: Vec<DefaultStatementV1>,
        call: Box<DefaultExpressionV1>,
        definition_origin: ExportDefinitionSourceV1,
    },
    Bind {
        source: DefaultBindingTemporaryV1,
        target: crate::DefaultBindingLeafV1,
        definition_origin: ExportDefinitionSourceV1,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultBindingActionViewV1<'a> {
    Project {
        source: &'a DefaultBindingTemporaryV1,
        result: &'a DefaultBindingTemporaryV1,
        projection: &'a DefaultBindingProjectionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Component {
        source: &'a DefaultBindingTemporaryV1,
        index: NonZeroU32,
        result: &'a DefaultBindingTemporaryV1,
        setup: &'a [DefaultStatementV1],
        call: &'a DefaultExpressionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Bind {
        source: &'a DefaultBindingTemporaryV1,
        target: &'a crate::DefaultBindingLeafV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
}

impl DefaultBindingActionV1 {
    pub const fn project(
        source: DefaultBindingTemporaryV1,
        result: DefaultBindingTemporaryV1,
        projection: DefaultBindingProjectionV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self(DefaultBindingActionKindV1::Project {
            source,
            result,
            projection,
            definition_origin,
        })
    }

    pub fn try_component(
        source: DefaultBindingTemporaryV1,
        index: NonZeroU32,
        result: DefaultBindingTemporaryV1,
        setup: Vec<DefaultStatementV1>,
        call: DefaultExpressionV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Result<Self, DefaultBindingActionBuildError> {
        u32::try_from(setup.len()).map_err(|_| DefaultBindingActionBuildError::TooManySetup)?;
        Ok(Self(DefaultBindingActionKindV1::Component {
            source,
            index,
            result,
            setup,
            call: Box::new(call),
            definition_origin,
        }))
    }

    pub const fn bind(
        source: DefaultBindingTemporaryV1,
        target: crate::DefaultBindingLeafV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self(DefaultBindingActionKindV1::Bind {
            source,
            target,
            definition_origin,
        })
    }

    pub fn view(&self) -> DefaultBindingActionViewV1<'_> {
        match &self.0 {
            DefaultBindingActionKindV1::Project {
                source,
                result,
                projection,
                definition_origin,
            } => DefaultBindingActionViewV1::Project {
                source,
                result,
                projection,
                definition_origin,
            },
            DefaultBindingActionKindV1::Component {
                source,
                index,
                result,
                setup,
                call,
                definition_origin,
            } => DefaultBindingActionViewV1::Component {
                source,
                index: *index,
                result,
                setup,
                call,
                definition_origin,
            },
            DefaultBindingActionKindV1::Bind {
                source,
                target,
                definition_origin,
            } => DefaultBindingActionViewV1::Bind {
                source,
                target,
                definition_origin,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBindingPlanV1 {
    subject: DefaultBindingTemporaryV1,
    shape: DefaultBindingShapeV1,
    actions: Vec<DefaultBindingActionV1>,
}

impl DefaultBindingPlanV1 {
    pub fn try_new(
        subject: DefaultBindingTemporaryV1,
        shape: DefaultBindingShapeV1,
        actions: Vec<DefaultBindingActionV1>,
    ) -> Result<Self, DefaultBindingPlanBuildError> {
        u32::try_from(actions.len()).map_err(|_| DefaultBindingPlanBuildError::TooManyActions)?;
        Ok(Self {
            subject,
            shape,
            actions,
        })
    }

    pub const fn subject(&self) -> &DefaultBindingTemporaryV1 {
        &self.subject
    }

    pub const fn shape(&self) -> &DefaultBindingShapeV1 {
        &self.shape
    }

    pub fn actions(&self) -> &[DefaultBindingActionV1] {
        &self.actions
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultIteratorConformanceV1 {
    source: DefaultBindingTemporaryV1,
    iterator: DefaultBindingTemporaryV1,
    interface_type: SignatureTypeKey,
    definition_origin: ExportDefinitionSourceV1,
}

impl DefaultIteratorConformanceV1 {
    pub const fn new(
        source: DefaultBindingTemporaryV1,
        iterator: DefaultBindingTemporaryV1,
        interface_type: SignatureTypeKey,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            source,
            iterator,
            interface_type,
            definition_origin,
        }
    }

    pub const fn source(&self) -> &DefaultBindingTemporaryV1 {
        &self.source
    }

    pub const fn iterator(&self) -> &DefaultBindingTemporaryV1 {
        &self.iterator
    }

    pub const fn interface_type(&self) -> &SignatureTypeKey {
        &self.interface_type
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultAppliedOptionV1 {
    some_payload: DefaultEnumVariantFieldRefV1,
    none: DefaultEnumVariantRefV1,
}

impl DefaultAppliedOptionV1 {
    pub const fn new(
        some_payload: DefaultEnumVariantFieldRefV1,
        none: DefaultEnumVariantRefV1,
    ) -> Self {
        Self { some_payload, none }
    }

    pub const fn some_payload(&self) -> &DefaultEnumVariantFieldRefV1 {
        &self.some_payload
    }

    pub const fn none(&self) -> &DefaultEnumVariantRefV1 {
        &self.none
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultIteratorNextV1 {
    callable: DefaultCallableRefV1,
    result: DefaultBindingTemporaryV1,
    option: DefaultAppliedOptionV1,
    element: DefaultBindingTemporaryV1,
    definition_origin: ExportDefinitionSourceV1,
}

impl DefaultIteratorNextV1 {
    pub const fn new(
        callable: DefaultCallableRefV1,
        result: DefaultBindingTemporaryV1,
        option: DefaultAppliedOptionV1,
        element: DefaultBindingTemporaryV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            callable,
            result,
            option,
            element,
            definition_origin,
        }
    }

    pub const fn callable(&self) -> &DefaultCallableRefV1 {
        &self.callable
    }

    pub const fn result(&self) -> &DefaultBindingTemporaryV1 {
        &self.result
    }

    pub const fn option(&self) -> &DefaultAppliedOptionV1 {
        &self.option
    }

    pub const fn element(&self) -> &DefaultBindingTemporaryV1 {
        &self.element
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultForIterationPlanV1 {
    source_setup: Vec<DefaultStatementV1>,
    source: DefaultBindingTemporaryV1,
    source_init: Box<DefaultExpressionV1>,
    iterator_setup: Vec<DefaultStatementV1>,
    iterator_call: Box<DefaultExpressionV1>,
    conformance: DefaultIteratorConformanceV1,
    next: DefaultIteratorNextV1,
    binding: DefaultBindingPlanV1,
    body: Vec<DefaultStatementV1>,
}

impl DefaultForIterationPlanV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        source_setup: Vec<DefaultStatementV1>,
        source: DefaultBindingTemporaryV1,
        source_init: DefaultExpressionV1,
        iterator_setup: Vec<DefaultStatementV1>,
        iterator_call: DefaultExpressionV1,
        conformance: DefaultIteratorConformanceV1,
        next: DefaultIteratorNextV1,
        binding: DefaultBindingPlanV1,
        body: Vec<DefaultStatementV1>,
    ) -> Result<Self, DefaultForIterationPlanBuildError> {
        require_for_statements(
            &source_setup,
            DefaultForIterationPlanBuildError::TooManySourceSetup,
        )?;
        require_for_statements(
            &iterator_setup,
            DefaultForIterationPlanBuildError::TooManyIteratorSetup,
        )?;
        require_for_statements(
            &body,
            DefaultForIterationPlanBuildError::TooManyBodyStatements,
        )?;
        Ok(Self {
            source_setup,
            source,
            source_init: Box::new(source_init),
            iterator_setup,
            iterator_call: Box::new(iterator_call),
            conformance,
            next,
            binding,
            body,
        })
    }

    pub fn source_setup(&self) -> &[DefaultStatementV1] {
        &self.source_setup
    }

    pub const fn source(&self) -> &DefaultBindingTemporaryV1 {
        &self.source
    }

    pub const fn source_init(&self) -> &DefaultExpressionV1 {
        &self.source_init
    }

    pub fn iterator_setup(&self) -> &[DefaultStatementV1] {
        &self.iterator_setup
    }

    pub const fn iterator_call(&self) -> &DefaultExpressionV1 {
        &self.iterator_call
    }

    pub const fn conformance(&self) -> &DefaultIteratorConformanceV1 {
        &self.conformance
    }

    pub const fn next(&self) -> &DefaultIteratorNextV1 {
        &self.next
    }

    pub const fn binding(&self) -> &DefaultBindingPlanV1 {
        &self.binding
    }

    pub fn body(&self) -> &[DefaultStatementV1] {
        &self.body
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBindingActionBuildError {
    TooManySetup,
}

impl fmt::Display for DefaultBindingActionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySetup => {
                formatter.write_str("default binding component setup count exceeds u32")
            }
        }
    }
}

impl std::error::Error for DefaultBindingActionBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBindingPlanBuildError {
    TooManyActions,
}

impl fmt::Display for DefaultBindingPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyActions => formatter.write_str("default binding action count exceeds u32"),
        }
    }
}

impl std::error::Error for DefaultBindingPlanBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultForIterationPlanBuildError {
    TooManySourceSetup,
    TooManyIteratorSetup,
    TooManyBodyStatements,
}

impl fmt::Display for DefaultForIterationPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySourceSetup => {
                formatter.write_str("default for source setup count exceeds u32")
            }
            Self::TooManyIteratorSetup => {
                formatter.write_str("default for iterator setup count exceeds u32")
            }
            Self::TooManyBodyStatements => {
                formatter.write_str("default for body statement count exceeds u32")
            }
        }
    }
}

impl std::error::Error for DefaultForIterationPlanBuildError {}

fn require_for_statements(
    statements: &[DefaultStatementV1],
    error: DefaultForIterationPlanBuildError,
) -> Result<(), DefaultForIterationPlanBuildError> {
    u32::try_from(statements.len())
        .map(|_| ())
        .map_err(|_| error)
}
