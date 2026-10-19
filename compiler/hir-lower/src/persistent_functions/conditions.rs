use super::*;
use scoop_identity::{SignatureConditionKey as Condition, SignatureTypeKey};

impl FunctionIdentityBuilder<'_> {
    pub(super) fn function_conditions(
        &self,
        function: hir::FunctionId,
        binders: &[SignatureBinder],
    ) -> Result<Vec<Condition>, PersistentFunctionIdentityError> {
        let signature = &self.lowerer.signatures[&function];
        let mut conditions = Vec::new();
        for parameter in &signature.type_params[signature.owner_type_param_count..] {
            let binder = binders
                .iter()
                .find(|binder| binder.parameter == parameter.id)
                .ok_or_else(|| self.failure(function, Detail::InvalidBinderInheritance))?;
            let subject = SignatureTypeKey::Binder {
                depth: binder.depth,
                index: binder.index,
            };
            let map = |ty| {
                self.type_mapper
                    .map(ty, binders)
                    .map_err(|error| self.failure(function, Detail::InvalidSignatureType(error)))
            };
            let subjects = [(
                subject.clone(),
                &parameter.bounds,
                &parameter.excluded_interfaces,
            )]
            .into_iter()
            .chain(parameter.companion.as_ref().map(|companion| {
                (
                    SignatureTypeKey::Companion(Box::new(subject.clone())),
                    &companion.bounds,
                    &companion.excluded_interfaces,
                )
            }));
            if parameter.companion.is_some() {
                conditions.push(Condition::Companion { subject });
            }
            for (subject, bounds, excluded) in subjects {
                match bounds {
                    hir::TypeParamBounds::Unconstrained => {}
                    hir::TypeParamBounds::Value { .. } => conditions.push(Condition::Value {
                        subject: subject.clone(),
                    }),
                    hir::TypeParamBounds::Ref { .. } => {
                        if !matches!(subject, SignatureTypeKey::Companion(_)) {
                            conditions.push(Condition::Ref {
                                subject: subject.clone(),
                            });
                        }
                    }
                    hir::TypeParamBounds::Nominal(bounds) => {
                        for bound in bounds.in_source_order() {
                            conditions.push(Condition::Upper {
                                subject: subject.clone(),
                                bound: map(bound.ty())?,
                            });
                        }
                    }
                }
                for bound in excluded {
                    conditions.push(Condition::ExcludedInterface {
                        subject: subject.clone(),
                        bound: map(bound.ty)?,
                    });
                }
            }
        }
        conditions.sort_unstable();
        Ok(conditions)
    }
}
