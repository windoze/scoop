use super::*;

mod callables;
mod enums;
mod nominals;

use callables::{
    protocol_callable_source, validate_interface_dispatch_relations,
    validate_protocol_callable_owners,
};
use enums::{validate_foreign_callback_relations, validate_option_relations};
use nominals::validate_nominal_relations;

pub(super) fn validate_foundation_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    validate_nominal_relations(surface, foundation)?;
    validate_option_relations(surface, foundation)?;
    validate_foreign_callback_relations(surface, foundation)?;
    validate_protocol_callable_owners(surface, foundation)?;
    validate_interface_dispatch_relations(surface, foundation)?;
    for (kind, callable) in surface.fixed_intrinsic_callables() {
        validate_operation_owner(surface, foundation, kind, callable)?;
    }
    Ok(())
}

fn validate_operation_owner(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
    kind: IntrinsicFunctionKind,
    callable: &CoreProtocolCallableV1,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source = protocol_callable_source(foundation, callable.definition()).ok_or(
        CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::OperationCallableKindMismatch(kind),
        ),
    )?;
    let expected = expected_operation_owner(surface, kind);
    let owners = source.owners().owners();
    let matches = match expected {
        None => owners.is_empty(),
        Some(expected) => owners == [expected],
    };
    if matches {
        Ok(())
    } else {
        Err(CoreCompilerProtocolSurfaceValidationError::OperationOwnerMismatch(kind))
    }
}
