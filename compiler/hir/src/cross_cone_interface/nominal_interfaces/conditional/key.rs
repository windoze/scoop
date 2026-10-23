//! Reconstruct the declaration key in its exported binder domain.

use super::*;
use crate::{ConditionalInterfaceKey, SourceNominalId, TypeParameterBoundsV1};
use scoop_identity::SignatureConditionKey as Condition;

impl ConditionalInterfaceRecordV1 {
    pub fn key(&self, owner: SourceNominalId) -> ConditionalInterfaceKey {
        let mut conditions = Vec::new();
        for (index, parameter) in self.parameters.binders().iter().enumerate() {
            let subject = SignatureTypeKey::Binder {
                depth: 0,
                index: index as u32,
            };
            conditions.extend(
                parameter
                    .requirements()
                    .iter()
                    .cloned()
                    .map(Condition::Member),
            );
            if parameter.companion().is_some() {
                conditions.push(Condition::Companion {
                    subject: subject.clone(),
                });
            }
            for (bounds, excluded, subject) in [(
                parameter.bounds(),
                parameter.excluded_interfaces(),
                subject.clone(),
            )]
            .into_iter()
            .chain(parameter.companion().map(|companion| {
                (
                    companion.bounds(),
                    companion.excluded_interfaces(),
                    SignatureTypeKey::Companion(Box::new(subject)),
                )
            })) {
                match bounds {
                    TypeParameterBoundsV1::Unconstrained => {}
                    TypeParameterBoundsV1::Value => conditions.push(Condition::Value {
                        subject: subject.clone(),
                    }),
                    TypeParameterBoundsV1::Ref => {
                        if !matches!(subject, SignatureTypeKey::Companion(_)) {
                            conditions.push(Condition::Ref {
                                subject: subject.clone(),
                            });
                        }
                    }
                    TypeParameterBoundsV1::Nominal(bounds) => {
                        for bound in bounds
                            .class()
                            .into_iter()
                            .chain(bounds.interfaces().values())
                        {
                            conditions.push(Condition::Upper {
                                subject: subject.clone(),
                                bound: bound.clone(),
                            });
                        }
                    }
                }
                conditions.extend(excluded.values().iter().cloned().map(|bound| {
                    Condition::ExcludedInterface {
                        subject: subject.clone(),
                        bound,
                    }
                }));
            }
        }
        conditions.sort_unstable();
        ConditionalInterfaceKey {
            owner,
            interface: self.interface.clone(),
            conditions,
        }
    }
}
