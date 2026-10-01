//! Record locations are separate from the actual coroutine protocol identities.

use super::*;

pub(super) struct Declarations {
    pub continuation: export::SourceNominalId,
    pub suspend_task: export::SourceNominalId,
    pub suspend_registration: export::SourceNominalId,
    pub start_coroutine: FunctionSource,
    pub suspend_coroutine: FunctionSource,
    pub continuation_resume: FunctionSource,
    pub continuation_resume_with_exception: FunctionSource,
    pub suspend_task_run: FunctionSource,
    pub suspend_registration_register: FunctionSource,
}

impl Concretizer<'_> {
    pub(super) fn coroutine_declarations(&self) -> Declarations {
        match self.core {
            export::CoreProtocols::Defined(core) => {
                let core = core.coroutines;
                Declarations {
                    continuation: self.source.nominal_identities[core.continuation]
                        .declaration_id(),
                    suspend_task: self.source.nominal_identities[core.suspend_task]
                        .declaration_id(),
                    suspend_registration: self.source.nominal_identities[core.suspend_registration]
                        .declaration_id(),
                    start_coroutine: FunctionSource::Local(core.start_coroutine),
                    suspend_coroutine: FunctionSource::Local(core.suspend_coroutine),
                    continuation_resume: FunctionSource::Local(core.continuation_resume),
                    continuation_resume_with_exception: FunctionSource::Local(
                        core.continuation_resume_with_exception,
                    ),
                    suspend_task_run: FunctionSource::Local(core.suspend_task_run),
                    suspend_registration_register: FunctionSource::Local(
                        core.suspend_registration_register,
                    ),
                }
            }
            export::CoreProtocols::Imported(core) => {
                let core = core.coroutines();
                Declarations {
                    continuation: export::SourceNominalId::GenericTemplate(
                        core.continuation().persistent(),
                    ),
                    suspend_task: export::SourceNominalId::GenericTemplate(
                        core.suspend_task().persistent(),
                    ),
                    suspend_registration: export::SourceNominalId::GenericTemplate(
                        core.suspend_registration().persistent(),
                    ),
                    start_coroutine: self.imported_coroutine_function(core.start_coroutine()),
                    suspend_coroutine: self.imported_coroutine_function(core.suspend_coroutine()),
                    continuation_resume: self
                        .imported_coroutine_function(core.continuation_resume()),
                    continuation_resume_with_exception: self
                        .imported_coroutine_function(core.continuation_resume_with_exception()),
                    suspend_task_run: self.imported_coroutine_function(core.suspend_task_run()),
                    suspend_registration_register: self
                        .imported_coroutine_function(core.suspend_registration_register()),
                }
            }
        }
    }

    fn imported_coroutine_function(
        &self,
        callable: &export::ImportedCoreProtocolCallable,
    ) -> FunctionSource {
        let declaration = match callable.definition() {
            export::ImportedCoreProtocolCallableDefinition::Function(id) => {
                export::DefaultCallableDeclarationV1::Function(id.persistent())
            }
            export::ImportedCoreProtocolCallableDefinition::GenericFunction(id) => {
                export::DefaultCallableDeclarationV1::GenericFunction(id.persistent())
            }
            _ => unreachable!("coroutine roles name source functions"),
        };
        let source = self
            .source
            .imported_generic_templates
            .iter()
            .find_map(|(id, template)| {
                (template.declaration.body_owner() == declaration).then_some(id)
            })
            .expect("Export HIR retains every coroutine protocol signature");
        FunctionSource::Imported(source)
    }

    pub(super) fn request_protocol_function(
        &mut self,
        source: FunctionSource,
        owner: Option<concrete::MethodOwner>,
        result: concrete::TypeId,
    ) -> concrete::FunctionId {
        let key = self.function_key(source, owner, vec![result]);
        self.request_function_key(key, source)
    }
}
