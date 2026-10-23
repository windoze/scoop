use super::*;
use scoop_identity::{SignatureConditionKey as Condition, SignatureTypeKey};

pub(crate) fn signature_parameter_conditions<E>(
    parameters: &[hir::TypeParamDecl],
    binders: &[SignatureBinder],
    mut map: impl FnMut(hir::TypeId) -> Result<SignatureTypeKey, E>,
) -> Result<Vec<Condition>, E> {
    let mut conditions = Vec::new();
    for parameter in parameters {
        let binder = binders
            .iter()
            .find(|binder| binder.parameter == parameter.id)
            .expect("condition parameters belong to the supplied signature binder");
        let subject = SignatureTypeKey::Binder {
            depth: binder.depth,
            index: binder.index,
        };
        for requirement in &parameter.requirements {
            conditions.push(Condition::Member(requirement.contract.key(&mut map)?));
        }
        let subjects = [(
            &parameter.bounds,
            &parameter.excluded_interfaces,
            subject.clone(),
        )]
        .into_iter()
        .chain(parameter.companion.as_ref().map(|companion| {
            (
                &companion.bounds,
                &companion.excluded_interfaces,
                SignatureTypeKey::Companion(Box::new(subject.clone())),
            )
        }));
        if parameter.companion.is_some() {
            conditions.push(Condition::Companion { subject });
        }
        for (bounds, excluded, subject) in subjects {
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
