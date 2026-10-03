//! Bind runtime function signature operands to their exact type identities.

use scoop_identity::{
    CoreBuiltinNominal, Effect, ExactTypeDiagnosticCatalog, ExactTypeDiagnosticGraph, ExactTypeKey,
    GeneratedNominalKey, PersistentExactTypeId, SourceDeclarationKey, SourceDeclarationKind,
    ValidatedIdentityGraph,
};
use scoop_lir::{ConeProductionSectionV2, StrongTypeDescriptorRefV2, TypeDescriptorRelations};

use super::SharedLirStrongProductionError as Error;

pub(super) fn validate(
    production: &ConeProductionSectionV2,
    graph: &ValidatedIdentityGraph,
) -> Result<(), Error> {
    let operands = ExactTypeDiagnosticCatalog::try_new(graph, &[])?;
    for registration in production.type_registrations().registrations() {
        let descriptor = registration.semantic();
        let exact = descriptor.exact_type();
        let key = graph.canonical_key::<_, ExactTypeKey>(exact)?;
        let is_interface = interface_type(&operands, key.as_ref());
        match (key.as_ref(), descriptor.relations()) {
            (_, TypeDescriptorRelations::Interface { parents }) if is_interface => {
                let mut previous = None;
                for parent in parents {
                    let Some(parent) = parent else {
                        return Err(invalid(exact, "interface_parent"));
                    };
                    let parent = parent.exact_type();
                    if previous.is_some_and(|previous| previous >= parent)
                        || !operands
                            .exact_type_key(parent)
                            .is_some_and(|key| interface_type(&operands, key))
                    {
                        return Err(invalid(exact, "interface_parent"));
                    }
                    previous = Some(parent);
                }
            }
            (
                ExactTypeKey::Function {
                    effect,
                    parameters,
                    result,
                },
                TypeDescriptorRelations::Signature {
                    is_suspend,
                    parameters: actual_parameters,
                    result: actual_result,
                },
            ) => {
                if (*effect == Effect::Suspend) != *is_suspend
                    || parameters.len() != actual_parameters.len()
                {
                    return Err(invalid(exact, "signature"));
                }
                for (expected, actual) in parameters
                    .iter()
                    .zip(actual_parameters)
                    .chain(std::iter::once((result, actual_result)))
                {
                    if !operand_matches(&operands, *expected, *actual)? {
                        return Err(invalid(exact, "signature_operand"));
                    }
                }
            }
            (ExactTypeKey::Function { .. }, TypeDescriptorRelations::Absent)
            | (_, TypeDescriptorRelations::Signature { .. })
            | (_, TypeDescriptorRelations::Interface { .. }) => {
                return Err(invalid(exact, "kind"));
            }
            (_, TypeDescriptorRelations::Absent) if is_interface => {
                return Err(invalid(exact, "kind"));
            }
            (_, TypeDescriptorRelations::Absent) => {}
        }
    }
    Ok(())
}

fn interface_type(graph: &impl ExactTypeDiagnosticGraph, key: &ExactTypeKey) -> bool {
    let declaration = match key {
        ExactTypeKey::Nominal(id) => graph.source_type_declaration(*id),
        ExactTypeKey::NominalApplication { origin, .. } => {
            graph.source_generic_type_declaration(*origin)
        }
        _ => None,
    };
    declaration.is_some_and(|declaration| {
        declaration.declaration_kind() == SourceDeclarationKind::Interface
    })
}

fn operand_matches(
    graph: &impl ExactTypeDiagnosticGraph,
    expected: PersistentExactTypeId,
    actual: Option<StrongTypeDescriptorRefV2>,
) -> Result<bool, Error> {
    let key = graph
        .exact_type_key(expected)
        .ok_or_else(|| invalid(expected, "operand_type"))?;
    if *key == ExactTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id()) {
        return Ok(actual.is_none());
    }
    let Some(actual) = actual else {
        return Ok(false);
    };
    let is_value = match key {
        ExactTypeKey::Tuple(_)
        | ExactTypeKey::RawPointer(_)
        | ExactTypeKey::NativeFunctionPointer { .. } => true,
        ExactTypeKey::Nominal(nominal) => graph
            .source_type_declaration(*nominal)
            .is_some_and(value_declaration),
        ExactTypeKey::NominalApplication { origin, .. } => graph
            .source_generic_type_declaration(*origin)
            .is_some_and(value_declaration),
        ExactTypeKey::Function { .. } => false,
    };
    if !is_value {
        return Ok(actual.exact_type() == expected);
    }
    let actual = graph
        .exact_type_key(actual.exact_type())
        .ok_or_else(|| invalid(expected, "operand_descriptor"))?;
    let ExactTypeKey::Nominal(nominal) = *actual else {
        return Ok(false);
    };
    Ok(matches!(
        graph.generated_nominal_key(nominal),
        Some(GeneratedNominalKey::BoxedValue { payload }) if *payload == expected
    ))
}

fn value_declaration(declaration: &SourceDeclarationKey) -> bool {
    matches!(
        declaration.declaration_kind(),
        SourceDeclarationKind::Struct | SourceDeclarationKind::Enum
    )
}

fn invalid(exact: PersistentExactTypeId, field: &'static str) -> Error {
    Error::FunctionDescriptor { exact, field }
}
