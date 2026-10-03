//! Structural reduction from type relations to variable bounds.

use scoop_hir as hir;

use super::constraints::{
    CallableCategory, CallableParameter, CallableReturn, CallableShape, CallableShapeMismatch,
    Constraint, ConstraintFailure, ConstraintFailureKind, ConstraintOrigin, ConstraintRecord,
    InferenceSession, InferenceVariableId, NominalApplication, RelationKind, TypeTerm,
};
use crate::{Lowerer, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AtomicConstraintKind {
    Exact(InferenceVariableId, TypeTerm),
    LowerBound(InferenceVariableId, TypeTerm),
    UpperBound(InferenceVariableId, TypeTerm),
    Kind(InferenceVariableId, hir::TypeParamKind),
    ClassBound(InferenceVariableId, TypeTerm),
    Implements(InferenceVariableId, TypeTerm),
    CallableShape(CallableShape, TypeTerm),
    ConcreteApplication(NominalApplication),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AtomicConstraint {
    pub(super) kind: AtomicConstraintKind,
    pub(super) origin: ConstraintOrigin,
}

pub(super) fn reduce_constraints(
    lowerer: &mut Lowerer,
    session: &InferenceSession,
) -> Result<Vec<AtomicConstraint>, ConstraintFailure> {
    let mut reducer = RelationReducer {
        lowerer,
        session,
        atomic: Vec::new(),
    };
    for record in session.constraints() {
        reducer.reduce(record)?;
    }
    Ok(reducer.atomic)
}

pub(super) fn reduce_relation(
    lowerer: &mut Lowerer,
    session: &InferenceSession,
    relation: RelationKind,
    left: TypeTerm,
    right: TypeTerm,
    origin: ConstraintOrigin,
) -> Result<Vec<AtomicConstraint>, ConstraintFailure> {
    let mut reducer = RelationReducer {
        lowerer,
        session,
        atomic: Vec::new(),
    };
    match relation {
        RelationKind::Equal => reducer.equal(left, right, origin)?,
        RelationKind::Subtype => reducer.subtype(left, right, origin)?,
    }
    Ok(reducer.atomic)
}

struct RelationReducer<'a> {
    lowerer: &'a mut Lowerer,
    session: &'a InferenceSession,
    atomic: Vec<AtomicConstraint>,
}

impl RelationReducer<'_> {
    fn reduce(&mut self, record: &ConstraintRecord) -> Result<(), ConstraintFailure> {
        match &record.constraint {
            Constraint::Equal(left, right) => self.equal(*left, *right, record.origin),
            Constraint::Subtype(left, right) => self.subtype(*left, *right, record.origin),
            Constraint::CallableShape(shape, expected) => {
                self.callable_shape(shape, *expected, record.origin)
            }
            Constraint::Kind(variable, kind) => {
                self.check_variable(*variable, record.origin)?;
                if *kind != hir::TypeParamKind::Any {
                    self.push(AtomicConstraintKind::Kind(*variable, *kind), record.origin);
                }
                Ok(())
            }
            Constraint::Implements(variable, interface) => {
                self.check_variable(*variable, record.origin)?;
                self.push(
                    AtomicConstraintKind::Implements(*variable, *interface),
                    record.origin,
                );
                Ok(())
            }
            Constraint::ClassBound(variable, class) => {
                self.check_variable(*variable, record.origin)?;
                self.push(
                    AtomicConstraintKind::ClassBound(*variable, *class),
                    record.origin,
                );
                Ok(())
            }
            Constraint::ConcreteApplication(application) => {
                self.push(
                    AtomicConstraintKind::ConcreteApplication(application.clone()),
                    record.origin,
                );
                Ok(())
            }
        }
    }

    fn equal(
        &mut self,
        left: TypeTerm,
        right: TypeTerm,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        match (self.root(left, origin)?, self.root(right, origin)?) {
            (RelationRoot::Variable(left), RelationRoot::Variable(right)) if left == right => {
                Ok(())
            }
            (RelationRoot::Variable(left), RelationRoot::Variable(right)) => {
                self.push(
                    AtomicConstraintKind::Exact(left, TypeTerm::Variable(right)),
                    origin,
                );
                self.push(
                    AtomicConstraintKind::Exact(right, TypeTerm::Variable(left)),
                    origin,
                );
                Ok(())
            }
            (RelationRoot::Variable(variable), _) => {
                self.push(AtomicConstraintKind::Exact(variable, right), origin);
                Ok(())
            }
            (_, RelationRoot::Variable(variable)) => {
                self.push(AtomicConstraintKind::Exact(variable, left), origin);
                Ok(())
            }
            (RelationRoot::Type(left_ty), RelationRoot::Type(right_ty)) => {
                self.equal_types(left, left_ty, right, right_ty, origin)
            }
        }
    }

    fn equal_types(
        &mut self,
        left: TypeTerm,
        left_ty: hir::TypeId,
        right: TypeTerm,
        right_ty: hir::TypeId,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        if let (Some(left_application), Some(right_application)) = (
            self.lowerer.nominal_application(left_ty),
            self.lowerer.nominal_application(right_ty),
        ) {
            return self.equal_nominal_arguments(
                left_application.template == right_application.template,
                left,
                right,
                left_application.arguments,
                right_application.arguments,
                origin,
            );
        }
        match (
            self.lowerer.types[left_ty].clone(),
            self.lowerer.types[right_ty].clone(),
        ) {
            (Type::Tuple(left_types), Type::Tuple(right_types))
                if left_types.len() == right_types.len() =>
            {
                left_types
                    .into_iter()
                    .zip(right_types)
                    .try_for_each(|(left_ty, right_ty)| {
                        self.equal(
                            nested_term(left, left_ty),
                            nested_term(right, right_ty),
                            origin,
                        )
                    })
            }
            (Type::Function(left_id), Type::Function(right_id))
            | (Type::FunPtr(left_id), Type::FunPtr(right_id)) => {
                let left_signature = self.lowerer.function_types[left_id].clone();
                let right_signature = self.lowerer.function_types[right_id].clone();
                if left_signature.is_suspend != right_signature.is_suspend
                    || left_signature.parameter_types.len() != right_signature.parameter_types.len()
                {
                    return Err(self.relation_failure(RelationKind::Equal, left, right, origin));
                }
                for (left_ty, right_ty) in left_signature
                    .parameter_types
                    .into_iter()
                    .zip(right_signature.parameter_types)
                {
                    self.equal(
                        nested_term(left, left_ty),
                        nested_term(right, right_ty),
                        origin,
                    )?;
                }
                self.equal(
                    nested_term(left, left_signature.return_type),
                    nested_term(right, right_signature.return_type),
                    origin,
                )
            }
            (Type::Ptr(left_ty), Type::Ptr(right_ty)) => self.equal(
                nested_term(left, left_ty),
                nested_term(right, right_ty),
                origin,
            ),
            _ if self.lowerer.types_equal(left_ty, right_ty) => Ok(()),
            _ => Err(self.relation_failure(RelationKind::Equal, left, right, origin)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn equal_nominal_arguments(
        &mut self,
        same_template: bool,
        left_term: TypeTerm,
        right_term: TypeTerm,
        left_arguments: Vec<hir::TypeId>,
        right_arguments: Vec<hir::TypeId>,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        if !same_template || left_arguments.len() != right_arguments.len() {
            return Err(self.relation_failure(RelationKind::Equal, left_term, right_term, origin));
        }
        left_arguments
            .into_iter()
            .zip(right_arguments)
            .try_for_each(|(left, right)| {
                self.equal(
                    nested_term(left_term, left),
                    nested_term(right_term, right),
                    origin,
                )
            })
    }

    fn subtype(
        &mut self,
        left: TypeTerm,
        right: TypeTerm,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        match (self.root(left, origin)?, self.root(right, origin)?) {
            (RelationRoot::Variable(left), RelationRoot::Variable(right)) if left == right => {
                Ok(())
            }
            (RelationRoot::Variable(variable), _) => {
                self.push(AtomicConstraintKind::UpperBound(variable, right), origin);
                Ok(())
            }
            (_, RelationRoot::Variable(variable)) => {
                self.push(AtomicConstraintKind::LowerBound(variable, left), origin);
                Ok(())
            }
            (RelationRoot::Type(left_ty), RelationRoot::Type(right_ty)) => {
                self.subtype_types(left, left_ty, right, right_ty, origin)
            }
        }
    }

    fn subtype_types(
        &mut self,
        left: TypeTerm,
        left_ty: hir::TypeId,
        right: TypeTerm,
        right_ty: hir::TypeId,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        if matches!(self.lowerer.types[right_ty], Type::Any) {
            return Ok(());
        }
        if let Some(target) = self.lowerer.nominal_application(right_ty) {
            let mut pending = vec![left_ty];
            let mut seen = std::collections::HashSet::new();
            while let Some(candidate) = pending.pop() {
                if !seen.insert(candidate) {
                    continue;
                }
                if let Some(actual) = self.lowerer.nominal_application(candidate)
                    && actual.template == target.template
                {
                    return self.equal_nominal_arguments(
                        true,
                        left,
                        right,
                        actual.arguments,
                        target.arguments,
                        origin,
                    );
                }
                pending.extend(self.lowerer.direct_nominal_supertypes(candidate));
            }
            return Err(self.relation_failure(RelationKind::Subtype, left, right, origin));
        }
        match (
            self.lowerer.types[left_ty].clone(),
            self.lowerer.types[right_ty].clone(),
        ) {
            (Type::Function(left_id), Type::Function(right_id)) => {
                let left_signature = self.lowerer.function_types[left_id].clone();
                let right_signature = self.lowerer.function_types[right_id].clone();
                if left_signature.is_suspend != right_signature.is_suspend
                    || left_signature.parameter_types.len() != right_signature.parameter_types.len()
                {
                    return Err(self.relation_failure(RelationKind::Subtype, left, right, origin));
                }
                for (left_parameter, right_parameter) in left_signature
                    .parameter_types
                    .into_iter()
                    .zip(right_signature.parameter_types)
                {
                    self.subtype(
                        nested_term(right, right_parameter),
                        nested_term(left, left_parameter),
                        origin,
                    )?;
                }
                self.subtype(
                    nested_term(left, left_signature.return_type),
                    nested_term(right, right_signature.return_type),
                    origin,
                )
            }
            (Type::FunPtr(left_id), Type::FunPtr(right_id)) => {
                let left_signature = self.lowerer.function_types[left_id].clone();
                let right_signature = self.lowerer.function_types[right_id].clone();
                if left_signature.is_suspend != right_signature.is_suspend
                    || left_signature.parameter_types.len() != right_signature.parameter_types.len()
                {
                    return Err(self.relation_failure(RelationKind::Subtype, left, right, origin));
                }
                for (left_parameter, right_parameter) in left_signature
                    .parameter_types
                    .into_iter()
                    .zip(right_signature.parameter_types)
                {
                    self.equal(
                        nested_term(left, left_parameter),
                        nested_term(right, right_parameter),
                        origin,
                    )?;
                }
                self.equal(
                    nested_term(left, left_signature.return_type),
                    nested_term(right, right_signature.return_type),
                    origin,
                )
            }
            (Type::Ptr(left_ty), Type::Ptr(right_ty)) => self.equal(
                nested_term(left, left_ty),
                nested_term(right, right_ty),
                origin,
            ),
            (Type::Tuple(left_types), Type::Tuple(right_types))
                if left_types.len() == right_types.len() =>
            {
                left_types
                    .into_iter()
                    .zip(right_types)
                    .try_for_each(|(left_ty, right_ty)| {
                        self.equal(
                            nested_term(left, left_ty),
                            nested_term(right, right_ty),
                            origin,
                        )
                    })
            }
            _ if !term_contains_session_parameter(self.lowerer, self.session, left)
                && !term_contains_session_parameter(self.lowerer, self.session, right)
                && self.lowerer.is_subtype(left_ty, right_ty) =>
            {
                Ok(())
            }
            _ => Err(self.relation_failure(RelationKind::Subtype, left, right, origin)),
        }
    }

    fn callable_shape(
        &mut self,
        shape: &CallableShape,
        expected: TypeTerm,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        self.push(
            AtomicConstraintKind::CallableShape(shape.clone(), expected),
            origin,
        );
        let RelationRoot::Type(expected_ty) = self.root(expected, origin)? else {
            return Ok(());
        };
        let signature = match (shape.category, self.lowerer.types[expected_ty].clone()) {
            (CallableCategory::Managed, Type::Function(signature))
            | (CallableCategory::Native, Type::FunPtr(signature)) => {
                self.lowerer.function_types[signature].clone()
            }
            _ => {
                return Err(ConstraintFailure {
                    origin,
                    kind: ConstraintFailureKind::CallableShape(
                        CallableShapeMismatch::ExpectedCallable,
                    ),
                });
            }
        };
        if shape.is_suspend != signature.is_suspend {
            return Err(ConstraintFailure {
                origin,
                kind: ConstraintFailureKind::CallableShape(CallableShapeMismatch::Suspend),
            });
        }
        if shape.parameters.len() != signature.parameter_types.len() {
            return Err(ConstraintFailure {
                origin,
                kind: ConstraintFailureKind::CallableShape(CallableShapeMismatch::Arity {
                    expected: signature.parameter_types.len(),
                    actual: shape.parameters.len(),
                }),
            });
        }
        for (actual, expected_parameter) in shape.parameters.iter().zip(signature.parameter_types) {
            if let CallableParameter::Explicit(actual) = actual {
                self.subtype(nested_term(expected, expected_parameter), *actual, origin)?;
            }
        }
        if let CallableReturn::Explicit(actual) = shape.return_type {
            self.subtype(actual, nested_term(expected, signature.return_type), origin)?;
        }
        Ok(())
    }

    fn root(
        &self,
        term: TypeTerm,
        origin: ConstraintOrigin,
    ) -> Result<RelationRoot, ConstraintFailure> {
        match term {
            TypeTerm::Variable(variable) => {
                self.check_variable(variable, origin)?;
                Ok(RelationRoot::Variable(variable))
            }
            TypeTerm::Type(ty) => match self.lowerer.types[ty] {
                Type::Param(parameter) => self
                    .session
                    .variable_for(parameter)
                    .map(RelationRoot::Variable)
                    .ok_or(ConstraintFailure {
                        origin,
                        kind: ConstraintFailureKind::ForeignTypeParameter(parameter),
                    }),
                _ => Ok(RelationRoot::Type(ty)),
            },
            TypeTerm::Rigid(ty) => Ok(RelationRoot::Type(ty)),
        }
    }

    fn check_variable(
        &self,
        variable: InferenceVariableId,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        if self.session.variable_index(variable).is_some() {
            Ok(())
        } else {
            Err(ConstraintFailure {
                origin,
                kind: ConstraintFailureKind::ForeignVariable(variable),
            })
        }
    }

    fn push(&mut self, kind: AtomicConstraintKind, origin: ConstraintOrigin) {
        self.atomic.push(AtomicConstraint { kind, origin });
    }

    fn relation_failure(
        &self,
        relation: RelationKind,
        left: TypeTerm,
        right: TypeTerm,
        origin: ConstraintOrigin,
    ) -> ConstraintFailure {
        ConstraintFailure {
            origin,
            kind: ConstraintFailureKind::Relation {
                relation,
                left,
                right,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RelationRoot {
    Variable(InferenceVariableId),
    Type(hir::TypeId),
}

fn nested_term(parent: TypeTerm, ty: hir::TypeId) -> TypeTerm {
    match parent {
        TypeTerm::Type(_) => TypeTerm::Type(ty),
        TypeTerm::Rigid(_) => TypeTerm::Rigid(ty),
        TypeTerm::Variable(_) => {
            unreachable!("a variable relation is reduced before structural decomposition")
        }
    }
}

fn term_contains_session_parameter(
    lowerer: &Lowerer,
    session: &InferenceSession,
    term: TypeTerm,
) -> bool {
    match term {
        TypeTerm::Type(ty) => type_contains_session_parameter(lowerer, session, ty),
        TypeTerm::Rigid(_) | TypeTerm::Variable(_) => false,
    }
}

pub(crate) fn type_contains_session_parameter(
    lowerer: &Lowerer,
    session: &InferenceSession,
    ty: hir::TypeId,
) -> bool {
    if let Some((_, arguments)) = lowerer.dependency_nominal_application(ty) {
        return arguments
            .iter()
            .any(|argument| type_contains_session_parameter(lowerer, session, *argument));
    }
    let children: Vec<hir::TypeId> = match &lowerer.types[ty] {
        Type::Param(parameter) => return session.variable_for(*parameter).is_some(),
        Type::Struct(application) => lowerer.struct_applications[*application].arguments.clone(),
        Type::Class(application) => lowerer.class_applications[*application].arguments.clone(),
        Type::Interface(application) => lowerer.interface_applications[*application]
            .arguments
            .clone(),
        Type::Enum(application) => lowerer.enum_applications[*application].arguments.clone(),
        Type::Tuple(elements) => elements.clone(),
        Type::Function(signature) | Type::FunPtr(signature) => {
            let signature = &lowerer.function_types[*signature];
            let mut children = signature.parameter_types.clone();
            children.push(signature.return_type);
            children
        }
        Type::Ptr(pointee) => vec![*pointee],
        Type::Unit | Type::Integer(_) | Type::Boolean | Type::String | Type::Any => Vec::new(),
    };
    children
        .into_iter()
        .any(|child| type_contains_session_parameter(lowerer, session, child))
}
