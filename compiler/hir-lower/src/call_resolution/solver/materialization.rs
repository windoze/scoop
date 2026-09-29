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
        if let Some((declaration, arguments)) =
            self.types[ty].clone().imported_nominal_application()
        {
            let Some(arguments) = self.materialize_types(session, bindings, arguments, origin)?
            else {
                return Ok(None);
            };
            return self
                .imported_nominal_application(declaration.owner(), arguments)
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
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                let Some(arguments) =
                    self.materialize_types(session, bindings, &application.arguments, origin)?
                else {
                    return Ok(None);
                };
                if Some(application.template) == self.ffi_fun_ptr {
                    let [function] = arguments.as_slice() else {
                        return Ok(None);
                    };
                    let Type::Function(function) = self.types[*function] else {
                        return Ok(None);
                    };
                    return Ok(Some(self.intern_type(Type::FunPtr(function))));
                }
                Ok(Some(
                    self.struct_application(application.template, arguments),
                ))
            }
            Type::Class(application) => {
                let application = self.class_applications[application].clone();
                let Some(arguments) =
                    self.materialize_types(session, bindings, &application.arguments, origin)?
                else {
                    return Ok(None);
                };
                Ok(Some(
                    self.class_application(application.template, arguments),
                ))
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let Some(arguments) =
                    self.materialize_types(session, bindings, &application.arguments, origin)?
                else {
                    return Ok(None);
                };
                Ok(Some(self.intern_interface_application(
                    application.template,
                    arguments,
                )))
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                let Some(arguments) =
                    self.materialize_types(session, bindings, &application.arguments, origin)?
                else {
                    return Ok(None);
                };
                Ok(Some(self.enum_application(application.template, arguments)))
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
        let terms = match application {
            NominalApplication::Struct(_, terms)
            | NominalApplication::Class(_, terms)
            | NominalApplication::Enum(_, terms)
            | NominalApplication::Imported(_, terms) => terms,
        };
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
        Ok(match application {
            NominalApplication::Struct(template, _) if Some(*template) == self.ffi_ptr => {
                let [pointee] = arguments.as_slice() else {
                    return Err(ConstraintFailure {
                        origin,
                        kind: ConstraintFailureKind::NonConcreteApplication(application.clone()),
                    });
                };
                self.intern_type(Type::Ptr(*pointee))
            }
            NominalApplication::Struct(template, _) if Some(*template) == self.ffi_fun_ptr => {
                let [function] = arguments.as_slice() else {
                    return Err(ConstraintFailure {
                        origin,
                        kind: ConstraintFailureKind::NonConcreteApplication(application.clone()),
                    });
                };
                let Type::Function(function) = self.types[*function] else {
                    return Err(ConstraintFailure {
                        origin,
                        kind: ConstraintFailureKind::NonConcreteApplication(application.clone()),
                    });
                };
                self.intern_type(Type::FunPtr(function))
            }
            NominalApplication::Struct(template, _) => {
                self.struct_application(*template, arguments)
            }
            NominalApplication::Class(template, _) => self.class_application(*template, arguments),
            NominalApplication::Enum(template, _) => self.enum_application(*template, arguments),
            NominalApplication::Imported(owner, _) => self
                .imported_nominal_application(*owner, arguments)
                .map_err(|_| ConstraintFailure {
                    origin,
                    kind: ConstraintFailureKind::NonConcreteApplication(application.clone()),
                })?,
        })
    }
}
