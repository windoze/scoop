//! Identity resolution shared by legacy and foundation-scoped readers.

use super::*;

pub(super) mod foundation;

impl DecodedStrongDigestFinalizationPlanV1 {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        foundation: &crate::ConeLirFoundation,
    ) -> Result<StrongDigestFinalizationPlanV1, StrongDigestPlanValidationError> {
        self.validate_resolved(identities, foundation)
    }

    fn validate_resolved<R: scoop_identity::DigestOwnerResolver<IdentityReferenceError>>(
        self,
        identities: &mut R,
        foundation: &crate::ConeLirFoundation,
    ) -> Result<StrongDigestFinalizationPlanV1, StrongDigestPlanValidationError> {
        let mut resolved = Vec::with_capacity(self.nodes.len());
        for decoded in self.nodes {
            let key = (*decoded.identity.key())
                .resolve(identities)
                .map_err(StrongDigestPlanValidationError::NodeKey)?;
            let expected = DigestNodeId::from_key(&key)
                .map_err(StrongDigestPlanValidationError::IdentityHash)?;
            let id = decoded
                .identity
                .decoded_id()
                .verify(expected)
                .map_err(StrongDigestPlanValidationError::NodeIdentity)?;
            let identity = CborIdentityRecord::from_key(key)
                .map_err(StrongDigestPlanValidationError::IdentityHash)?;
            debug_assert_eq!(identity.id(), id);
            resolved.push((identity, decoded.direct_inputs, decoded.patch_intents));
        }

        for index in 1..resolved.len() {
            let previous = (
                resolved[index - 1].0.key().kind().tag(),
                resolved[index - 1].0.id(),
            );
            let current = (resolved[index].0.key().kind().tag(), resolved[index].0.id());
            if previous >= current {
                return Err(if previous == current {
                    StrongDigestPlanValidationError::Plan(DigestPlanError::DuplicateNode(current.1))
                } else {
                    StrongDigestPlanValidationError::NonCanonicalNodeOrder { index }
                });
            }
        }

        let known = resolved
            .iter()
            .map(|(identity, _, _)| {
                (
                    *identity.id().as_array(),
                    (identity.id(), identity.key().kind()),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let mut nodes = Vec::with_capacity(resolved.len());
        for (identity, decoded_inputs, decoded_patches) in resolved {
            let node = identity.id();
            let mut direct_inputs: Vec<DigestInputRefV1> = Vec::with_capacity(decoded_inputs.len());
            for (index, decoded) in decoded_inputs.into_iter().enumerate() {
                let raw = *decoded.decoded_node().as_array();
                let Some((target, actual_kind)) = known.get(&raw).copied() else {
                    return Err(StrongDigestPlanValidationError::MissingInput { node, input: raw });
                };
                if decoded.kind() != actual_kind {
                    return Err(StrongDigestPlanValidationError::InputKindMismatch {
                        node,
                        input: target,
                        declared: decoded.kind(),
                        actual: actual_kind,
                    });
                }
                let input = DigestInputRefV1::from_parts(actual_kind, target);
                if let Some(previous) = direct_inputs.last()
                    && previous.sort_key() >= input.sort_key()
                {
                    return Err(if previous.sort_key() == input.sort_key() {
                        StrongDigestPlanValidationError::Plan(DigestPlanError::DuplicateInput {
                            node,
                            input: target,
                        })
                    } else {
                        StrongDigestPlanValidationError::NonCanonicalInputOrder { node, index }
                    });
                }
                direct_inputs.push(input);
            }

            let mut patch_intents: Vec<
                CborIdentityRecord<DigestPatchIntentId, DigestPatchIntentKey>,
            > = Vec::with_capacity(decoded_patches.len());
            for (index, decoded) in decoded_patches.into_iter().enumerate() {
                let decoded_key = *decoded.key();
                let raw_source = *decoded_key.decoded_source().as_array();
                let Some((source, _)) = known.get(&raw_source).copied() else {
                    return Err(StrongDigestPlanValidationError::MissingPatchSource {
                        node,
                        source: raw_source,
                    });
                };
                let target_definition = identities
                    .resolve(decoded_key.target_definition())
                    .map_err(StrongDigestPlanValidationError::PatchTarget)?;
                let key = DigestPatchIntentKey::new(
                    source,
                    target_definition,
                    decoded_key.atom_role(),
                    decoded_key.semantic_field_role(),
                );
                let expected = DigestPatchIntentId::from_key(&key)
                    .map_err(StrongDigestPlanValidationError::IdentityHash)?;
                let id = decoded
                    .decoded_id()
                    .verify(expected)
                    .map_err(StrongDigestPlanValidationError::PatchIdentity)?;
                if let Some(previous) = patch_intents.last()
                    && previous.id() >= id
                {
                    return Err(if previous.id() == id {
                        StrongDigestPlanValidationError::Plan(
                            DigestPlanError::DuplicatePatchIntent { node, intent: id },
                        )
                    } else {
                        StrongDigestPlanValidationError::NonCanonicalPatchOrder { node, index }
                    });
                }
                let record = CborIdentityRecord::from_key(key)
                    .map_err(StrongDigestPlanValidationError::IdentityHash)?;
                debug_assert_eq!(record.id(), id);
                patch_intents.push(record);
            }
            nodes.push(DigestNodeV1 {
                identity,
                direct_inputs,
                patch_intents,
            });
        }

        validate_plan(&nodes, foundation).map_err(StrongDigestPlanValidationError::Plan)?;
        Ok(StrongDigestFinalizationPlanV1 { nodes })
    }
}
