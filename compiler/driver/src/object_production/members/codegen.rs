//! Consume Scoop object bytes and read the external C compiler's objects.

use super::*;

pub(in crate::object_production) fn plan_codegen_objects(
    target: LirTargetProfile,
    producer_units: &ProducerUnitPartitionV1,
    members: Vec<scoop_codegen::EmittedConeObjectMemberV1>,
    generated_c_bridge: &EmittedGeneratedCBridgeObjectSetV1,
) -> Result<PlannedObjectBindings, BuiltinObjectProductionError> {
    let mut scoop_lir_sources = Vec::with_capacity(members.len());
    for member in members {
        let scoop_codegen::EmittedConeObjectMemberV1 { units, bytes, .. } = member;
        scoop_lir_sources.push(UnboundScoopLirObject {
            units: units.definition_plans().to_vec(),
            bytes,
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

    plan_objects(
        target,
        producer_units,
        scoop_lir_sources,
        generated_c_bridge_sources,
    )
}
