//! A companion projection is reduced only after its host is known. It never
//! adds an inverse constraint from a companion argument to the host variable.

use super::*;
use crate::Type;
use crate::call_resolution::constraints::RelationKind;
use crate::call_resolution::relations::reduce_relation;
use std::collections::HashSet;

impl Lowerer {
    pub(super) fn resolve_projection_checks(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        checks: &[AtomicConstraint],
        resolved: &mut HashSet<usize>,
    ) -> Result<Vec<AtomicConstraint>, ConstraintFailure> {
        let mut derived = Vec::new();
        for (index, check) in checks.iter().enumerate() {
            if resolved.contains(&index) {
                continue;
            }
            match &check.kind {
                AtomicConstraintKind::Companion(variable, bounds) => {
                    let Some(host) = self.materialize_term(
                        session,
                        bindings,
                        TypeTerm::Variable(*variable),
                        check.origin,
                    )?
                    else {
                        continue;
                    };
                    let companion = self.require_companion(*variable, host, check.origin)?;
                    if let hir::TypeParamBounds::Nominal(bounds) = &bounds.bounds {
                        for bound in bounds.in_source_order() {
                            derived.extend(reduce_relation(
                                self,
                                session,
                                RelationKind::Subtype,
                                TypeTerm::Rigid(companion),
                                TypeTerm::Type(bound.ty()),
                                check.origin,
                            )?);
                        }
                    }
                    resolved.insert(index);
                }
                AtomicConstraintKind::DeferredRelation(relation, left, right) => {
                    let Some(left) =
                        self.resolve_projection_term(session, bindings, *left, check.origin)?
                    else {
                        continue;
                    };
                    let Some(right) =
                        self.resolve_projection_term(session, bindings, *right, check.origin)?
                    else {
                        continue;
                    };
                    derived.extend(reduce_relation(
                        self,
                        session,
                        *relation,
                        left,
                        right,
                        check.origin,
                    )?);
                    resolved.insert(index);
                }
                _ => {}
            }
        }
        Ok(derived)
    }

    fn resolve_projection_term(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        term: TypeTerm,
        origin: ConstraintOrigin,
    ) -> Result<Option<TypeTerm>, ConstraintFailure> {
        if matches!(term, TypeTerm::Type(ty) if matches!(self.types[ty], Type::Companion(_))) {
            return self
                .materialize_term(session, bindings, term, origin)
                .map(|ty| ty.map(TypeTerm::Rigid));
        }
        Ok(Some(term))
    }

    fn require_companion(
        &mut self,
        variable: InferenceVariableId,
        host: hir::TypeId,
        origin: ConstraintOrigin,
    ) -> Result<hir::TypeId, ConstraintFailure> {
        self.projected_companion_type(host)
            .map_err(|message| ConstraintFailure {
                origin,
                kind: ConstraintFailureKind::Companion { variable, message },
            })
    }

    pub(super) fn validate_companion_condition(
        &mut self,
        session: &InferenceSession,
        bindings: &[Option<hir::TypeId>],
        variable: InferenceVariableId,
        bounds: &hir::CompanionBounds,
        origin: ConstraintOrigin,
    ) -> Result<(), ConstraintFailure> {
        let host = self.require_materialized(
            session,
            bindings,
            &Bound {
                term: TypeTerm::Variable(variable),
                origin,
            },
        )?;
        let companion = self.require_companion(variable, host, origin)?;
        for bound in &bounds.excluded_interfaces {
            let required = self.require_materialized(
                session,
                bindings,
                &Bound {
                    term: TypeTerm::Type(bound.ty),
                    origin,
                },
            )?;
            let outcome = self.excludes_interface(companion, required);
            if outcome != crate::types::InterfaceExclusion::Satisfied {
                return Err(ConstraintFailure {
                    origin,
                    kind: ConstraintFailureKind::Companion {
                        variable,
                        message: format!(
                            "companion `{}` does not satisfy `!{}`: {}",
                            self.type_name(companion),
                            self.type_name(required),
                            if outcome == crate::types::InterfaceExclusion::Implemented {
                                "the interface is implemented"
                            } else {
                                "the generic environment does not establish this negative fact"
                            }
                        ),
                    },
                });
            }
        }
        Ok(())
    }
}
