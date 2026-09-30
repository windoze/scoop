use super::*;
use crate::cross_cone_type_semantics::inheritance::is_nominal_ancestor;

type Relations = BTreeMap<(PersistentExactTypeId, PersistentExactTypeId), bool>;

impl Graph<'_> {
    pub(in super::super) fn receiver_is_subtype(
        &self,
        source: PersistentExactTypeId,
        target: PersistentExactTypeId,

        path: &WirePath,
    ) -> Result<bool, Error> {
        // Reuse shared subqueries within one occurrence, never across calls.
        self.receiver_relation(source, target, &mut Relations::new(), path)
    }

    fn receiver_relation(
        &self,
        source: PersistentExactTypeId,
        target: PersistentExactTypeId,
        known: &mut Relations,
        path: &WirePath,
    ) -> Result<bool, Error> {
        if let Some(result) = known.get(&(source, target)) {
            return Ok(*result);
        }

        let source_key = self
            .current
            .identities
            .canonical_key::<_, ExactTypeKey>(source)?;
        let target_key = self
            .current
            .identities
            .canonical_key::<_, ExactTypeKey>(target)?;
        let result = if source == target {
            true
        } else {
            match (source_key.as_ref(), target_key.as_ref()) {
                (_, ExactTypeKey::Nominal(owner))
                    if *owner == CoreBuiltinNominal::Any.identity_record().id() =>
                {
                    true
                }
                (
                    ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. },
                    ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. },
                ) => is_nominal_ancestor(source, target, path, |current| {
                    self.source_receiver_parents(current, path)
                })?,
                (
                    ExactTypeKey::Function {
                        effect: source_effect,
                        parameters: source_parameters,
                        result: source_result,
                    },
                    ExactTypeKey::Function {
                        effect: target_effect,
                        parameters: target_parameters,
                        result: target_result,
                    },
                ) if source_effect == target_effect
                    && source_parameters.len() == target_parameters.len() =>
                {
                    let mut compatible = true;
                    for (target, source) in target_parameters.iter().zip(source_parameters) {
                        if !self.receiver_relation(*target, *source, known, path)? {
                            compatible = false;
                            break;
                        }
                    }
                    if compatible {
                        self.receiver_relation(*source_result, *target_result, known, path)?
                    } else {
                        false
                    }
                }
                // Tuples and native pointers remain invariant; incompatible
                // kinds and function effects/arity have no subtype relation.
                _ => false,
            }
        };

        known.insert((source, target), result);
        Ok(result)
    }
}
