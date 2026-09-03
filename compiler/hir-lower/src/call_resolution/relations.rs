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
        match (
            self.lowerer.types[left_ty].clone(),
            self.lowerer.types[right_ty].clone(),
        ) {
            (Type::Struct(left), Type::Struct(right)) => {
                let left = self.lowerer.struct_applications[left].clone();
                let right = self.lowerer.struct_applications[right].clone();
                self.equal_nominal_arguments(
                    left.template == right.template,
                    left.arguments,
                    right.arguments,
                    origin,
                    left_ty,
                    right_ty,
                )
            }
            (Type::Class(left), Type::Class(right)) => {
                let left = self.lowerer.class_applications[left].clone();
                let right = self.lowerer.class_applications[right].clone();
                self.equal_nominal_arguments(
                    left.template == right.template,
                    left.arguments,
                    right.arguments,
                    origin,
                    left_ty,
                    right_ty,
                )
            }
            (Type::Interface(left), Type::Interface(right)) => {
                let left = self.lowerer.interface_applications[left].clone();
                let right = self.lowerer.interface_applications[right].clone();
                self.equal_nominal_arguments(
                    left.template == right.template,
                    left.arguments,
                    right.arguments,
                    origin,
                    left_ty,
                    right_ty,
                )
            }
            (Type::Enum(left), Type::Enum(right)) => {
                let left = self.lowerer.enum_applications[left].clone();
                let right = self.lowerer.enum_applications[right].clone();
                self.equal_nominal_arguments(
                    left.template == right.template,
                    left.arguments,
                    right.arguments,
                    origin,
                    left_ty,
                    right_ty,
                )
            }
            (Type::Tuple(left), Type::Tuple(right)) if left.len() == right.len() => left
                .into_iter()
                .zip(right)
                .try_for_each(|(left, right)| self.equal(left.into(), right.into(), origin)),
            (Type::Function(left), Type::Function(right))
            | (Type::FunPtr(left), Type::FunPtr(right)) => {
                let left = self.lowerer.function_types[left].clone();
                let right = self.lowerer.function_types[right].clone();
                if left.is_suspend != right.is_suspend
                    || left.parameter_types.len() != right.parameter_types.len()
                {
                    return Err(self.relation_failure(
                        RelationKind::Equal,
                        TypeTerm::Type(left_ty),
                        TypeTerm::Type(right_ty),
                        origin,
                    ));
                }
                for (left, right) in left.parameter_types.into_iter().zip(right.parameter_types) {
                    self.equal(left.into(), right.into(), origin)?;
                }
                self.equal(left.return_type.into(), right.return_type.into(), origin)
            }
            (Type::Ptr(left), Type::Ptr(right)) => self.equal(left.into(), right.into(), origin),
            _ if self.lowerer.types_equal(left_ty, right_ty) => Ok(()),
            _ => Err(self.relation_failure(RelationKind::Equal, left, right, origin)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn equal_nominal_arguments(
        &mut self,
        same_template: bool,
        left: Vec<hir::TypeId>,
        right: Vec<hir::TypeId>,
        origin: ConstraintOrigin,
        left_ty: hir::TypeId,
        right_ty: hir::TypeId,
    ) -> Result<(), ConstraintFailure> {
        if !same_template || left.len() != right.len() {
            return Err(self.relation_failure(
                RelationKind::Equal,
                left_ty.into(),
                right_ty.into(),
                origin,
            ));
        }
        left.into_iter()
            .zip(right)
            .try_for_each(|(left, right)| self.equal(left.into(), right.into(), origin))
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
        match (
            self.lowerer.types[left_ty].clone(),
            self.lowerer.types[right_ty].clone(),
        ) {
            (Type::Struct(left), Type::Struct(right)) => {
                let left = self.lowerer.struct_applications[left].clone();
                let right = self.lowerer.struct_applications[right].clone();
                self.equal_nominal_arguments(
                    left.template == right.template,
                    left.arguments,
                    right.arguments,
                    origin,
                    left_ty,
                    right_ty,
                )
            }
            (Type::Class(left_application_id), Type::Class(right_application_id)) => {
                let left_application = self.lowerer.class_applications[left_application_id].clone();
                let right_application =
                    self.lowerer.class_applications[right_application_id].clone();
                if left_application.template == right_application.template {
                    return self.equal_nominal_arguments(
                        true,
                        left_application.arguments,
                        right_application.arguments,
                        origin,
                        left_ty,
                        right_ty,
                    );
                }
                let Some((base, _)) = self.lowerer.classes[left_application.template]
                    .base_class
                    .clone()
                else {
                    return Err(self.relation_failure(RelationKind::Subtype, left, right, origin));
                };
                let base = self
                    .lowerer
                    .instantiate_ty(base, &left_application.arguments);
                self.subtype(base.into(), right, origin)
            }
            (Type::Enum(left), Type::Enum(right)) => {
                let left = self.lowerer.enum_applications[left].clone();
                let right = self.lowerer.enum_applications[right].clone();
                self.equal_nominal_arguments(
                    left.template == right.template,
                    left.arguments,
                    right.arguments,
                    origin,
                    left_ty,
                    right_ty,
                )
            }
            (Type::Interface(left_application_id), Type::Interface(right_application_id)) => {
                let left_application =
                    self.lowerer.interface_applications[left_application_id].clone();
                let right_application =
                    self.lowerer.interface_applications[right_application_id].clone();
                if left_application.template != right_application.template {
                    return self.subtype_via_interface(
                        left_ty,
                        right_application.template,
                        right,
                        origin,
                    );
                }
                let variances: Vec<_> = self.lowerer.interfaces[left_application.template]
                    .type_params
                    .iter()
                    .map(|parameter| parameter.variance)
                    .collect();
                for ((variance, left), right) in variances
                    .into_iter()
                    .zip(left_application.arguments)
                    .zip(right_application.arguments)
                {
                    match variance {
                        hir::Variance::Invariant => {
                            self.equal(left.into(), right.into(), origin)?
                        }
                        hir::Variance::Out => self.subtype(left.into(), right.into(), origin)?,
                        hir::Variance::In => self.subtype(right.into(), left.into(), origin)?,
                    }
                }
                Ok(())
            }
            (Type::Function(left), Type::Function(right)) => {
                let left = self.lowerer.function_types[left].clone();
                let right = self.lowerer.function_types[right].clone();
                if left.is_suspend != right.is_suspend
                    || left.parameter_types.len() != right.parameter_types.len()
                {
                    return Err(self.relation_failure(
                        RelationKind::Subtype,
                        TypeTerm::Type(left_ty),
                        TypeTerm::Type(right_ty),
                        origin,
                    ));
                }
                for (left, right) in left.parameter_types.into_iter().zip(right.parameter_types) {
                    self.subtype(right.into(), left.into(), origin)?;
                }
                self.subtype(left.return_type.into(), right.return_type.into(), origin)
            }
            (Type::FunPtr(left), Type::FunPtr(right)) => {
                let left = self.lowerer.function_types[left].clone();
                let right = self.lowerer.function_types[right].clone();
                if left.is_suspend != right.is_suspend
                    || left.parameter_types.len() != right.parameter_types.len()
                {
                    return Err(self.relation_failure(
                        RelationKind::Subtype,
                        left_ty.into(),
                        right_ty.into(),
                        origin,
                    ));
                }
                for (left, right) in left.parameter_types.into_iter().zip(right.parameter_types) {
                    self.equal(left.into(), right.into(), origin)?;
                }
                self.equal(left.return_type.into(), right.return_type.into(), origin)
            }
            (Type::Ptr(left), Type::Ptr(right)) => self.equal(left.into(), right.into(), origin),
            (Type::Tuple(left), Type::Tuple(right)) if left.len() == right.len() => left
                .into_iter()
                .zip(right)
                .try_for_each(|(left, right)| self.equal(left.into(), right.into(), origin)),
            (_, Type::Interface(right)) => {
                let right_application = self.lowerer.interface_applications[right].clone();
                self.subtype_via_interface(
                    left_ty,
                    right_application.template,
                    TypeTerm::Type(right_ty),
                    origin,
                )
            }
            _ if !self.lowerer.type_contains_param(left_ty)
                && !self.lowerer.type_contains_param(right_ty)
                && self.lowerer.is_subtype(left_ty, right_ty) =>
            {
                Ok(())
            }
            _ => Err(self.relation_failure(RelationKind::Subtype, left, right, origin)),
        }
    }

    fn subtype_via_interface(
        &mut self,
        left: hir::TypeId,
        target: hir::InterfaceId,
        right: TypeTerm,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        let Some(arguments) = self.interface_application_as(left, target) else {
            return Err(self.relation_failure(RelationKind::Subtype, left.into(), right, origin));
        };
        let implemented = self.lowerer.intern_interface_application(target, arguments);
        self.subtype(implemented.into(), right, origin)
    }

    fn interface_application_as(
        &mut self,
        ty: hir::TypeId,
        target: hir::InterfaceId,
    ) -> Option<Vec<hir::TypeId>> {
        let direct = match self.lowerer.types[ty].clone() {
            Type::Interface(_) => vec![ty],
            Type::Int => self
                .lowerer
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::Int),
            Type::UInt => self
                .lowerer
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::UInt),
            Type::Boolean => self
                .lowerer
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::Boolean),
            Type::String => self
                .lowerer
                .intrinsic_type_interfaces(hir::IntrinsicTypeKind::String),
            Type::Class(application) => self.lowerer.class_interfaces_for_application(application),
            Type::Struct(application) => {
                let application = self.lowerer.struct_applications[application].clone();
                self.lowerer.structs[application.template]
                    .interfaces
                    .clone()
                    .into_iter()
                    .map(|interface| {
                        self.lowerer
                            .instantiate_ty(interface, &application.arguments)
                    })
                    .collect()
            }
            Type::Enum(application) => {
                let application = self.lowerer.enum_applications[application].clone();
                self.lowerer.enums[application.template]
                    .interfaces
                    .clone()
                    .into_iter()
                    .map(|interface| {
                        self.lowerer
                            .instantiate_ty(interface, &application.arguments)
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        let mut closure = Vec::new();
        for interface in direct {
            self.lowerer
                .append_interface_closure(interface, &mut closure);
        }
        closure.into_iter().find_map(|interface| {
            let Type::Interface(application) = self.lowerer.types[interface] else {
                return None;
            };
            let application = &self.lowerer.interface_applications[application];
            (application.template == target).then(|| application.arguments.clone())
        })
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
        for (actual, expected) in shape.parameters.iter().zip(signature.parameter_types) {
            if let CallableParameter::Explicit(actual) = actual {
                self.subtype(expected.into(), *actual, origin)?;
            }
        }
        if let CallableReturn::Explicit(actual) = shape.return_type {
            self.subtype(actual, signature.return_type.into(), origin)?;
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
