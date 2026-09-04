//! Candidate-local inference identities and declarative constraints.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_hir as hir;

use super::arguments::SourceInputId;

static NEXT_SESSION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InferenceSessionId(u64);

impl InferenceSessionId {
    fn fresh() -> Self {
        Self(NEXT_SESSION.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InferenceEnvironmentId(u32);

impl InferenceEnvironmentId {
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct OwnerInferenceVariableId {
    session: InferenceSessionId,
    environment: InferenceEnvironmentId,
    index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CallableInferenceVariableId {
    session: InferenceSessionId,
    environment: InferenceEnvironmentId,
    index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InferenceVariableId {
    Owner(OwnerInferenceVariableId),
    Callable(CallableInferenceVariableId),
}

impl InferenceVariableId {
    pub(crate) const fn session(self) -> InferenceSessionId {
        match self {
            Self::Owner(variable) => variable.session,
            Self::Callable(variable) => variable.session,
        }
    }

    pub(crate) const fn group_index(self) -> usize {
        match self {
            Self::Owner(variable) => variable.index as usize,
            Self::Callable(variable) => variable.index as usize,
        }
    }
}

impl From<OwnerInferenceVariableId> for InferenceVariableId {
    fn from(value: OwnerInferenceVariableId) -> Self {
        Self::Owner(value)
    }
}

impl From<CallableInferenceVariableId> for InferenceVariableId {
    fn from(value: CallableInferenceVariableId) -> Self {
        Self::Callable(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeTerm {
    /// A declaration template. Every `Type::Param` reached through this term
    /// must belong to the current inference session and becomes a variable.
    Type(hir::TypeId),
    /// A type synthesized in the surrounding semantic context. Its type
    /// parameters are rigid inputs, even when nested in an application.
    Rigid(hir::TypeId),
    Variable(InferenceVariableId),
}

impl From<hir::TypeId> for TypeTerm {
    fn from(value: hir::TypeId) -> Self {
        Self::Rigid(value)
    }
}

impl From<InferenceVariableId> for TypeTerm {
    fn from(value: InferenceVariableId) -> Self {
        Self::Variable(value)
    }
}

impl From<OwnerInferenceVariableId> for TypeTerm {
    fn from(value: OwnerInferenceVariableId) -> Self {
        Self::Variable(value.into())
    }
}

impl From<CallableInferenceVariableId> for TypeTerm {
    fn from(value: CallableInferenceVariableId) -> Self {
        Self::Variable(value.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableParameter {
    Contextual,
    Explicit(TypeTerm),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableReturn {
    Contextual,
    Explicit(TypeTerm),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallableShape {
    pub(crate) category: CallableCategory,
    pub(crate) is_suspend: bool,
    pub(crate) parameters: Vec<CallableParameter>,
    pub(crate) return_type: CallableReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableCategory {
    Managed,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NominalApplication {
    Struct(hir::StructId, Vec<TypeTerm>),
    Class(hir::ClassId, Vec<TypeTerm>),
    Enum(hir::EnumId, Vec<TypeTerm>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Constraint {
    Equal(TypeTerm, TypeTerm),
    Subtype(TypeTerm, TypeTerm),
    CallableShape(CallableShape, TypeTerm),
    Kind(InferenceVariableId, hir::TypeParamKind),
    ClassBound(InferenceVariableId, TypeTerm),
    Implements(InferenceVariableId, TypeTerm),
    ConcreteApplication(NominalApplication),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstraintOrigin {
    Declaration,
    Receiver,
    Argument(SourceInputId),
    ExplicitTypeArgument(u32),
    ExpectedResult,
    TypeParameterBound(hir::TypeParamId),
    Specificity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstraintRecord {
    pub(crate) constraint: Constraint,
    pub(crate) origin: ConstraintOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelationKind {
    Equal,
    Subtype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallableShapeMismatch {
    ExpectedCallable,
    Suspend,
    Arity { expected: usize, actual: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConstraintFailureKind {
    ForeignVariable(InferenceVariableId),
    ForeignTypeParameter(hir::TypeParamId),
    Relation {
        relation: RelationKind,
        left: TypeTerm,
        right: TypeTerm,
    },
    CallableShape(CallableShapeMismatch),
    ConflictingExactBounds {
        variable: InferenceVariableId,
        first: hir::TypeId,
        second: hir::TypeId,
    },
    NoUniqueSolution {
        variable: InferenceVariableId,
        lower_bounds: Vec<hir::TypeId>,
        upper_bounds: Vec<hir::TypeId>,
        solution_frontier: Vec<hir::TypeId>,
    },
    Kind {
        variable: InferenceVariableId,
        solution: hir::TypeId,
        required: hir::TypeParamKind,
    },
    InterfaceBound {
        variable: InferenceVariableId,
        solution: hir::TypeId,
        required: hir::TypeId,
    },
    ClassBound {
        variable: InferenceVariableId,
        solution: hir::TypeId,
        required: hir::TypeId,
    },
    UnresolvedTerm(TypeTerm),
    NonConcreteApplication(NominalApplication),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstraintFailure {
    pub(crate) origin: ConstraintOrigin,
    pub(crate) kind: ConstraintFailureKind,
}

#[derive(Debug, Clone)]
struct InferenceEnvironment {
    owner_variables: Vec<OwnerInferenceVariableId>,
    callable_variables: Vec<CallableInferenceVariableId>,
}

/// One isolated inference graph. Environments let pairwise specificity put
/// both declarations in a single graph while retaining the owner/callable
/// distinction for each declaration.
#[derive(Debug, Clone)]
pub(crate) struct InferenceSession {
    id: InferenceSessionId,
    environments: Vec<InferenceEnvironment>,
    variables: Vec<InferenceVariableId>,
    by_parameter: HashMap<hir::TypeParamId, InferenceVariableId>,
    constraints: Vec<ConstraintRecord>,
}

impl InferenceSession {
    pub(crate) fn new() -> Self {
        Self {
            id: InferenceSessionId::fresh(),
            environments: Vec::new(),
            variables: Vec::new(),
            by_parameter: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    pub(crate) fn add_environment(
        &mut self,
        owner_parameters: &[hir::TypeParamDecl],
        callable_parameters: &[hir::TypeParamDecl],
    ) -> InferenceEnvironmentId {
        let environment = InferenceEnvironmentId(
            u32::try_from(self.environments.len())
                .expect("inference environment count exceeds u32"),
        );
        let owner_variables = owner_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let variable = OwnerInferenceVariableId {
                    session: self.id,
                    environment,
                    index: u32::try_from(index)
                        .expect("owner inference variable count exceeds u32"),
                };
                self.register_parameter(parameter.id, variable.into());
                variable
            })
            .collect();
        let callable_variables = callable_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let variable = CallableInferenceVariableId {
                    session: self.id,
                    environment,
                    index: u32::try_from(index)
                        .expect("callable inference variable count exceeds u32"),
                };
                self.register_parameter(parameter.id, variable.into());
                variable
            })
            .collect();
        self.environments.push(InferenceEnvironment {
            owner_variables,
            callable_variables,
        });
        environment
    }

    fn register_parameter(&mut self, parameter: hir::TypeParamId, variable: InferenceVariableId) {
        assert_eq!(variable.session(), self.id);
        assert!(
            self.by_parameter.insert(parameter, variable).is_none(),
            "one declaration type parameter cannot occur in two inference environments"
        );
        self.variables.push(variable);
    }

    pub(crate) fn owner_variables(
        &self,
        environment: InferenceEnvironmentId,
    ) -> &[OwnerInferenceVariableId] {
        &self.environments[environment.index()].owner_variables
    }

    pub(crate) fn callable_variables(
        &self,
        environment: InferenceEnvironmentId,
    ) -> &[CallableInferenceVariableId] {
        &self.environments[environment.index()].callable_variables
    }

    pub(crate) fn variable_for(&self, parameter: hir::TypeParamId) -> Option<InferenceVariableId> {
        self.by_parameter.get(&parameter).copied()
    }

    pub(crate) fn push(&mut self, constraint: Constraint, origin: ConstraintOrigin) {
        self.constraints
            .push(ConstraintRecord { constraint, origin });
    }

    pub(crate) const fn id(&self) -> InferenceSessionId {
        self.id
    }

    pub(crate) fn variables(&self) -> &[InferenceVariableId] {
        &self.variables
    }

    pub(crate) fn constraints(&self) -> &[ConstraintRecord] {
        &self.constraints
    }

    pub(crate) fn variable_index(&self, variable: InferenceVariableId) -> Option<usize> {
        (variable.session() == self.id)
            .then(|| {
                self.variables
                    .iter()
                    .position(|candidate| *candidate == variable)
            })
            .flatten()
    }
}

impl Default for InferenceSession {
    fn default() -> Self {
        Self::new()
    }
}
