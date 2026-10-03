//! Copy immutable codegen members while their temporary backing is alive.

use super::*;

pub(in crate::object_production) fn plan_codegen_objects(
    producer_units: &ProducerUnitPartitionV1,
    members: &[scoop_codegen::EmittedConeObjectMemberV1],
    generated_c_bridge: &EmittedGeneratedCBridgeObjectSetV1,
) -> Result<PlannedObjectBindings, BuiltinObjectProductionError> {
    let mut scoop_lir_sources = Vec::with_capacity(members.len());
    for member in members {
        let bytes = std::fs::read(member.path()).map_err(|source| {
            BuiltinObjectProductionError::ReadObject {
                producer: BuiltinObjectProducerV1::ScoopLir,
                path: member.path().to_path_buf(),
                source,
            }
        })?;
        let digest_patches = match member.kind() {
            EmittedConeObjectMemberKindV1::NonCallable { digest_patches, .. } => digest_patches
                .iter()
                .map(|materialization| {
                    UnboundDigestPatch::from_codegen(
                        materialization.location(),
                        materialization.checked_object_offset(),
                    )
                })
                .collect(),
            EmittedConeObjectMemberKindV1::CallableBody { .. } => Vec::new(),
        };
        scoop_lir_sources.push(UnboundScoopLirObject {
            units: member.units().definition_plans().to_vec(),
            bytes,
            digest_patches,
        });
    }

    let mut generated_c_bridge_sources = Vec::with_capacity(generated_c_bridge.members().len());
    for member in generated_c_bridge.members() {
        let bytes = std::fs::read(member.object_path()).map_err(|source| {
            BuiltinObjectProductionError::ReadObject {
                producer: BuiltinObjectProducerV1::GeneratedCBridge,
                path: member.object_path().to_path_buf(),
                source,
            }
        })?;
        generated_c_bridge_sources.push(UnboundGeneratedCBridgeObject {
            unit: member.unit(),
            bytes,
        });
    }

    let bindings = plan_objects(
        producer_units,
        scoop_lir_sources,
        generated_c_bridge_sources,
    )?;
    if bindings.digest_patches.is_empty() {
        return Err(BuiltinObjectProductionError::EmptyDigestMaterializationSet);
    }
    Ok(bindings)
}
