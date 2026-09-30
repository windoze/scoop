//! Substitution of a complete solver assignment back into canonical HIR types.

use scoop_hir as hir;

use super::Bound;
use crate::call_resolution::constraints::{
    ConstraintFailure, ConstraintFailureKind, ConstraintOrigin, InferenceSession,
    InferenceVariableId, NominalApplication, TypeTerm,
};
use crate::{Lowerer, Type};

impl Lowerer {
    pub(super) fn materialized_bounds(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        bounds: &[Bound],
    ) -> Result<Vec<hir::TypeId>, ConstraintFailure> {
        let mut result = Vec::new();
        for bound in bounds {
            if let Some(ty) = self.materialize_term(session, bindings, bound.term, bound.origin)? {
                // Keep one output per source bound: callers compare lengths to
                // distinguish a fully materialized set from one that still
                // depends on an unsolved variable. Duplicate bounds are valid.
                result.push(ty);
            }
        }
        Ok(result)
    }

    pub(super) fn require_materialized(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        bound: &Bound,
    ) -> Result<hir::TypeId, ConstraintFailure> {
        self.materialize_term(session, bindings, bound.term, bound.origin)?
            .ok_or(ConstraintFailure {
                origin: bound.origin,
                kind: ConstraintFailureKind::UnresolvedTerm(bound.term),
            })
    }

    pub(super) fn materialize_term(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        term: TypeTerm,
        origin: ConstraintOrigin,
    ) -> Result<Option<hir::TypeId>, ConstraintFailure> {
        match term {
            TypeTerm::Variable(variable) => self.binding_for(session, bindings, variable, origin),
            TypeTerm::Type(ty) => self.materialize_type(session, bindings, ty, origin),
            TypeTerm::Rigid(ty) => Ok(Some(ty)),
        }
    }

    fn materialize_type(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        ty: hir::TypeId,
        origin: ConstraintOrigin,
    ) -> Result<Option<hir::TypeId>, ConstraintFailure> {
        if let Some(application) = self.nominal_application(ty) {
            let Some(arguments) =
                self.materialize_types(session, bindings, &application.arguments, origin)?
            else {
                return Ok(None);
            };
            if self.nominal_intrinsic_kind(application.template)
                == Some(hir::IntrinsicTypeKind::FunPtr)
                && !matches!(arguments.as_slice(), [function] if matches!(self.types[*function], Type::Function(_)))
            {
                return Ok(None);
            }
            return self
                .apply_nominal_type(application.template, arguments)
                .map(Some)
                .map_err(|_| ConstraintFailure {
                    origin,
                    kind: ConstraintFailureKind::UnresolvedTerm(TypeTerm::Type(ty)),
                });
        }
        match self.types[ty].clone() {
            Type::Param(parameter) => {
                let variable = session.variable_for(parameter).ok_or(ConstraintFailure {
                    origin,
                    kind: ConstraintFailureKind::ForeignTypeParameter(parameter),
                })?;
                self.binding_for(session, bindings, variable, origin)
            }
            Type::Tuple(elements) => {
                let Some(elements) =
                    self.materialize_types(session, bindings, &elements, origin)?
                else {
                    return Ok(None);
                };
                Ok(Some(self.intern_type(Type::Tuple(elements))))
            }
            Type::Function(signature) => {
                let signature = self.function_types[signature].clone();
                let Some(parameters) =
                    self.materialize_types(session, bindings, &signature.parameter_types, origin)?
                else {
                    return Ok(None);
                };
                let Some(return_type) =
                    self.materialize_type(session, bindings, signature.return_type, origin)?
                else {
                    return Ok(None);
                };
                Ok(Some(self.intern_function_type(
                    signature.is_suspend,
                    parameters,
                    return_type,
                )))
            }
            Type::Ptr(pointee) => {
                let Some(pointee) = self.materialize_type(session, bindings, pointee, origin)?
                else {
                    return Ok(None);
                };
                Ok(Some(self.intern_type(Type::Ptr(pointee))))
            }
            Type::FunPtr(signature) => {
                let signature = self.function_types[signature].clone();
                let Some(parameters) =
                    self.materialize_types(session, bindings, &signature.parameter_types, origin)?
                else {
                    return Ok(None);
                };
                let Some(return_type) =
                    self.materialize_type(session, bindings, signature.return_type, origin)?
                else {
                    return Ok(None);
                };
                let function =
                    self.intern_function_type(signature.is_suspend, parameters, return_type);
                let Type::Function(signature) = self.types[function] else {
                    unreachable!("materialized function signature has function type")
                };
                Ok(Some(self.intern_type(Type::FunPtr(signature))))
            }
            _ => Ok(Some(ty)),
        }
    }

    fn materialize_types(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        types: &[hir::TypeId],
        origin: ConstraintOrigin,
    ) -> Result<Option<Vec<hir::TypeId>>, ConstraintFailure> {
        let mut result = Vec::with_capacity(types.len());
        for &ty in types {
            let Some(ty) = self.materialize_type(session, bindings, ty, origin)? else {
                return Ok(None);
            };
            result.push(ty);
        }
        Ok(Some(result))
    }

    fn binding_for(
        &self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        variable: InferenceVariableId,
        origin: ConstraintOrigin,
    ) -> Result<Option<hir::TypeId>, ConstraintFailure> {
        session
            .variable_index(variable)
            .map(|index| bindings[index])
            .ok_or(ConstraintFailure {
                origin,
                kind: ConstraintFailureKind::ForeignVariable(variable),
            })
    }

    pub(super) fn materialize_application(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        application: &NominalApplication,
        origin: ConstraintOrigin,
    ) -> Result<hir::TypeId, ConstraintFailure> {
        let terms = &application.arguments;
        let mut arguments = Vec::with_capacity(terms.len());
        for &term in terms {
            let Some(argument) = self.materialize_term(session, bindings, term, origin)? else {
                return Err(ConstraintFailure {
                    origin,
                    kind: ConstraintFailureKind::NonConcreteApplication(application.clone()),
                });
            };
            arguments.push(argument);
        }
        if self.nominal_intrinsic_kind(application.template) == Some(hir::IntrinsicTypeKind::FunPtr)
            && !matches!(arguments.as_slice(), [function] if matches!(self.types[*function], Type::Function(_)))
        {
            return Err(ConstraintFailure {
                origin,
                kind: ConstraintFailureKind::NonConcreteApplication(application.clone()),
            });
        }
        self.apply_nominal_type(application.template, arguments)
            .map_err(|_| ConstraintFailure {
                origin,
                kind: ConstraintFailureKind::NonConcreteApplication(application.clone()),
            })
    }
}
