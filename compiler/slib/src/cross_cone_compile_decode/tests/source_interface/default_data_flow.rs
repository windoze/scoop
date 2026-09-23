use scoop_hir::{
    DefaultLocalDataFlowLocalError, DefaultLocalDataFlowSiteV1,
    ExportDefaultLocalDataFlowValidationError,
};

use super::*;
use crate::CrossConeHirDefaultDataFlowError;

use super::default_fixture::{Case, fixture};

#[test]
fn the_ordinary_reader_accepts_a_default_local_defined_before_its_use() {
    let fixture = fixture(Case::Defined);
    let bytes = fixture.artifact();
    let front = validate_until_type_alias(&bytes)
        .validate_source_interfaces(vec![])
        .unwrap();
    assert_eq!(front.hir_interface().default_templates().records().len(), 1);
    assert_eq!(front.identity(), fixture.cone.identity());
}

#[test]
fn the_ordinary_reader_rejects_reading_a_later_default_local() {
    let fixture = fixture(Case::ReadBeforeDefinition);
    let bytes = fixture.artifact();
    let error = failure(&bytes, fixture.owner);
    assert!(matches!(
        error,
        ExportDefaultLocalDataFlowValidationError::Local {
            site: DefaultLocalDataFlowSiteV1::Expression,
            error: DefaultLocalDataFlowLocalError::UseBeforeDefinition,
            ..
        }
    ));
    assert!(error.to_string().contains("is used before its definition"));
}

#[test]
fn the_ordinary_reader_rejects_reassignment_of_an_immutable_default_local() {
    let fixture = fixture(Case::ImmutableAssignment);
    let bytes = fixture.artifact();
    assert!(matches!(
        failure(&bytes, fixture.owner),
        ExportDefaultLocalDataFlowValidationError::Local {
            site: DefaultLocalDataFlowSiteV1::Assignment,
            error: DefaultLocalDataFlowLocalError::Mutability { .. },
            ..
        }
    ));
}

#[test]
fn the_ordinary_reader_rejects_loop_control_outside_a_default_loop() {
    let fixture = fixture(Case::BreakOutsideLoop);
    let bytes = fixture.artifact();
    assert!(matches!(
        failure(&bytes, fixture.owner),
        ExportDefaultLocalDataFlowValidationError::LoopControlOutsideLoop { .. }
    ));
}

fn failure(
    bytes: &[u8],
    owner: CallableTemplateOrigin,
) -> ExportDefaultLocalDataFlowValidationError<crate::CrossConeHirDefaultFieldError> {
    let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultDataFlow(
        CrossConeHirDefaultDataFlowError::Template { index, key, source },
    )) = validate_until_type_alias(bytes).validate_source_interfaces(vec![])
    else {
        panic!("invalid default data flow must not pass the source-interface gate");
    };
    assert_eq!(index, 0);
    assert_eq!(key.owner(), owner);
    assert_eq!(key.parameter_position(), 0);
    *source
}
