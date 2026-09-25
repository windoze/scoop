use super::*;
use crate::cross_cone_type_semantics::inheritance::is_nominal_ancestor;

type Relations = BTreeMap<(PersistentExactTypeId, PersistentExactTypeId), bool>;

impl Graph<'_> {
    pub(super) fn receiver_is_subtype(
        &self,
        source: PersistentExactTypeId,
        target: PersistentExactTypeId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, Error> {
        // Reuse shared subqueries within one occurrence, never across calls.
        self.receiver_relation(source, target, 1, &mut Relations::new(), meter, path)
    }

    fn receiver_relation(
        &self,
        source: PersistentExactTypeId,
        target: PersistentExactTypeId,
        depth: u64,
        known: &mut Relations,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, Error> {
        meter.check_semantic_depth(depth, path)?;
        meter.charge_work(1 + u64::from(known.len().max(1).ilog2()), path)?;
        if let Some(result) = known.get(&(source, target)) {
            return Ok(*result);
        }
        meter.charge_nodes(1, path)?;
        meter.charge_work(
            2 * (1 + u64::from(self.current.identities.identity_count().max(1).ilog2())),
            path,
        )?;
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
                (ExactTypeKey::Nominal(_), ExactTypeKey::Nominal(_)) => {
                    is_nominal_ancestor(source, target, meter, path, |current, meter| {
                        self.source_receiver_parents(current, meter, path)
                    })?
                }
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
                        meter.charge_edges(1, path)?;
                        if !self.receiver_relation(
                            *target,
                            *source,
                            depth + 1,
                            known,
                            meter,
                            path,
                        )? {
                            compatible = false;
                            break;
                        }
                    }
                    if compatible {
                        meter.charge_edges(1, path)?;
                        self.receiver_relation(
                            *source_result,
                            *target_result,
                            depth + 1,
                            known,
                            meter,
                            path,
                        )?
                    } else {
                        false
                    }
                }
                // Tuples and native pointers remain invariant; incompatible
                // kinds and function effects/arity have no subtype relation.
                _ => false,
            }
        };
        meter.charge_work(1 + u64::from(known.len().max(1).ilog2()), path)?;
        meter.check_table_entries(known.len() as u64 + 1, path)?;
        meter.charge_collection_slots(1, path)?;
        known.insert((source, target), result);
        Ok(result)
    }
}
