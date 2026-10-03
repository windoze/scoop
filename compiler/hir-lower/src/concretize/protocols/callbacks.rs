use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_imported_callback_support(&mut self) {
        let export::CoreProtocols::Imported(core) = self.core else {
            return;
        };
        let callback = core.foreign_callbacks();
        let throwable =
            export::SourceNominalId::Concrete(core.exceptions().throwable().persistent());
        let mut required = Vec::new();
        for owner in [callback.mode().persistent(), callback.state().persistent()] {
            let application = self
                .source
                .enum_applications
                .values()
                .find(|application| {
                    application.template == export::SourceNominalId::Concrete(owner)
                })
                .expect("callback lowering retains its imported mode and state types");
            required.push(application.canonical_type);
        }
        let failure = self.source.enum_applications.values().find(|application| {
            application.template == export::SourceNominalId::GenericTemplate(core.option().option().persistent())
                && matches!(application.arguments.as_slice(), [ty] if matches!(self.source.types[*ty], export::Type::Class(class) if self.source.class_applications[class].template == throwable))
        }).expect("callback lowering retains the exact imported Option<Throwable> type");
        required.push(failure.canonical_type);
        for ty in required {
            self.lower_type(ty, &[]);
        }
    }
}
