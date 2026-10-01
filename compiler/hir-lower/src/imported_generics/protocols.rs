//! Core protocol declarations enter the ordinary template/signature queue.

use super::*;

impl Lowerer {
    pub(crate) fn prepare_coroutine_declarations(&mut self) -> Result<(), String> {
        let crate::CoreLoweringAuthority::Imported(core) = &self.core else {
            return Ok(());
        };
        let illegal_state = core.exceptions().illegal_state_exception().persistent();
        let protocols = core.coroutines();
        let definitions = [
            protocols.start_coroutine(),
            protocols.suspend_coroutine(),
            protocols.continuation_resume(),
            protocols.continuation_resume_with_exception(),
            protocols.suspend_task_run(),
            protocols.suspend_registration_register(),
        ]
        .map(|callable| match callable.definition() {
            hir::ImportedCoreProtocolCallableDefinition::Function(id) => {
                CallableTemplateOrigin::Function(id.persistent())
            }
            hir::ImportedCoreProtocolCallableDefinition::GenericFunction(id) => {
                CallableTemplateOrigin::GenericFunction(id.persistent())
            }
            _ => unreachable!("coroutine roles name source functions"),
        });
        for definition in definitions {
            let declaration = self
                .dependencies
                .as_ref()
                .expect("imported protocols have a dependency catalog")
                .callable_declaration(definition)
                .map_err(|error| error.to_string())?;
            self.request_imported_generic_template(declaration)?;
        }
        self.prepare_runtime_exception_type(illegal_state)
            .map_err(|error| format!("cannot resolve coroutine failure type: {error:?}"))
    }
}
