use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_imported_function(
        &mut self,
        source: export::ImportedGenericCallableTemplateId,
        arguments: &[concrete::TypeId],
    ) -> PendingFunction {
        let template = self.source.imported_generic_templates[source].clone();
        let (body, locals) = self.lower_body(&template.body, arguments);
        let parameters = template
            .parameters
            .iter()
            .map(|parameter| concrete::Param {
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, arguments),
                local: locals[parameter.local.into_raw().into_u32() as usize],
            })
            .collect();
        let receiver = match template.receiver {
            Some(ty) => concrete::FunctionReceiver::Extension(self.lower_type(ty, arguments)),
            None => concrete::FunctionReceiver::None,
        };
        PendingFunction {
            name: template.name.clone(),
            is_suspend: template.effects.execution() == scoop_identity::Effect::Suspend,
            modifiers: template.effects.callable_modifiers(),
            params: parameters,
            capture_parameters: Vec::new(),
            return_ty: self.lower_type(template.return_type, arguments),
            attributes: template.effects.function_attributes(),
            kind: concrete::FunctionKind::User(body),
            receiver,
            span: template.span,
        }
    }
}
