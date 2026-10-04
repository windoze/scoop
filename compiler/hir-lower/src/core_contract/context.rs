use super::*;

impl Lowerer {
    pub(super) fn missing_context_constructor(
        &mut self,
        files: &[ast::SourceFile],
        throwable: ClassId,
    ) -> Option<hir::ClassConstructorId> {
        let Some(crate::NominalTarget::Class(class)) =
            self.core_nominal_target("MissingContextException")
        else {
            let file = self.core_diagnostic_file();
            self.current_file = file;
            self.error(
                files[file].span,
                "scoop.core must define class `MissingContextException`".into(),
            );
            return None;
        };
        self.current_file = self.class_files[&class];
        let constructor = self.classes[class]
            .constructors
            .iter()
            .copied()
            .find(|constructor| {
                let parameters = &self.class_constructors[*constructor].parameters;
                parameters.len() == 1 && self.as_option(parameters[0].ty) == Some(self.string)
            });
        if self.classes[class].modifier != hir::ClassModifier::Final
            || !self.classes[class].type_params.is_empty()
            || !self.class_descends_from(class, throwable)
            || constructor.is_none()
        {
            self.error(self.classes[class].span, "`MissingContextException` must be a non-generic final Throwable subtype with a `(String?)` constructor".into());
            return None;
        }
        constructor
    }
}
