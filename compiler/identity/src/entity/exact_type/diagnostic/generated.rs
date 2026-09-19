//! The generated atom uses only the frozen role and persistent nominal id.

use super::*;
use crate::GeneratedNominalKey;

enum NominalDefinition<'a> {
    Source(&'a SourceDeclarationKey),
    Generated(&'a GeneratedNominalKey),
}

fn definition(
    id: PersistentTypeId,
    graph: &impl ExactTypeDiagnosticGraph,
) -> Result<NominalDefinition<'_>, ExactTypeDiagnosticError> {
    match (
        graph.source_type_declaration(id),
        graph.generated_nominal_key(id),
    ) {
        (Some(source), None) => Ok(NominalDefinition::Source(source)),
        (None, Some(generated)) => Ok(NominalDefinition::Generated(generated)),
        (Some(_), Some(_)) => Err(ExactTypeDiagnosticError::ConflictingNominalDefinitions(id)),
        (None, None) => Err(ExactTypeDiagnosticError::MissingSourceType(id)),
    }
}

pub(super) fn nominal_cost(
    id: PersistentTypeId,
    graph: &impl ExactTypeDiagnosticGraph,
    meter: &mut BudgetMeter,
) -> Result<usize, ExactTypeDiagnosticError> {
    match definition(id, graph)? {
        NominalDefinition::Source(source) => nominal_atom_cost(source, graph, meter),
        NominalDefinition::Generated(key) => {
            let path = WirePath::root();
            let bytes = scoop_wire::encode_canonical_temporary_with_meter(key, meter, &path)?;
            meter.charge_owned_bytes(bytes.len() as u64, &path)?;
            meter.charge_sha256((bytes.len() as u64).saturating_add(64), &path)?;
            if PersistentTypeId::from_generated_key(key).ok() != Some(id) {
                return Err(ExactTypeDiagnosticError::InvalidGeneratedNominal(id));
            }
            Ok("g(r=;i=)".len() + 8 + 64)
        }
    }
}

pub(super) fn write_nominal(
    id: PersistentTypeId,
    graph: &impl ExactTypeDiagnosticGraph,
    output: &mut String,
) -> Result<(), ExactTypeDiagnosticError> {
    match definition(id, graph)? {
        NominalDefinition::Source(source) => {
            super::render::write_nominal_atom(source, graph, output)
        }
        NominalDefinition::Generated(key) => {
            let tag: u32 = match key {
                GeneratedNominalKey::ClosureEnvironment { .. } => 1,
                GeneratedNominalKey::CallableAdapterEnvironment { .. } => 2,
                GeneratedNominalKey::CoroutineFrame { .. } => 3,
                GeneratedNominalKey::ContinuationAdapterEnvironment { .. } => 4,
                GeneratedNominalKey::CoroutineStep { .. } => 5,
                GeneratedNominalKey::BoxedValue { .. } => 6,
                GeneratedNominalKey::CoroutineSlot { .. } => 7,
                GeneratedNominalKey::ObjectBackingClass { .. } => 8,
            };
            output.push_str("g(r=");
            for byte in tag.to_be_bytes() {
                write_hex(byte, output);
            }
            output.push_str(";i=");
            for byte in id.as_array() {
                write_hex(*byte, output);
            }
            output.push(')');
            Ok(())
        }
    }
}

fn write_hex(byte: u8, output: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(char::from(HEX[(byte >> 4) as usize]));
    output.push(char::from(HEX[(byte & 15) as usize]));
}
