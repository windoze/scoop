use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_imported_function(
        &mut self,
        source: export::ImportedGenericCallableTemplateId,
        arguments: &[concrete::TypeId],
    ) -> PendingFunction {
        let template = self.source.imported_generic_templates[source].clone();
        let (body, locals) = self.lower_body(&template.body, arguments);
        let parameters: Vec<concrete::Param> = template
            .parameters
            .iter()
            .map(|parameter| concrete::Param {
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, arguments),
                local: locals[parameter.local.into_raw().into_u32() as usize],
            })
            .collect();
        let capture_parameters = match &template.declaration {
            export::ImportedCallableTemplateOrigin::Generic(_) => Vec::new(),
            export::ImportedCallableTemplateOrigin::Local { parent, descriptor } => descriptor
                .captures()
                .iter()
                .zip(&parameters)
                .map(|(capture, parameter)| concrete::LocalCaptureParameter {
                    binding: self.imported_capture_binding(*parent, capture.source()),
                    local: parameter.local,
                })
                .collect(),
        };
        let receiver = match template.receiver {
            Some(ty) => concrete::FunctionReceiver::Extension(self.lower_type(ty, arguments)),
            None => concrete::FunctionReceiver::None,
        };
        PendingFunction {
            name: template.name.clone(),
            is_suspend: template.effects.execution() == scoop_identity::Effect::Suspend,
            modifiers: template.effects.callable_modifiers(),
            params: parameters,
            capture_parameters,
            return_ty: self.lower_type(template.return_type, arguments),
            attributes: template.effects.function_attributes(),
            kind: concrete::FunctionKind::User(body),
            receiver,
            span: template.span,
        }
    }

    fn imported_capture_binding(
        &self,
        template: export::ImportedGenericCallableTemplateId,
        source: &export::DefaultCaptureSourceV1,
    ) -> concrete::BindingId {
        let template = &self.source.imported_generic_templates[template];
        let capture_index = match source {
            export::DefaultCaptureSourceV1::Local(selector) => {
                let (id, local) = template
                    .body
                    .locals
                    .iter()
                    .find(|(_, local)| &local.selector == selector)
                    .expect("provider captures retain their actual outer value selector");
                let index = match &template.declaration {
                    export::ImportedCallableTemplateOrigin::Generic(_) => None,
                    export::ImportedCallableTemplateOrigin::Local { descriptor, .. } => template
                        .parameters[..descriptor.capture_count() as usize]
                        .iter()
                        .position(|parameter| parameter.local == id),
                };
                match index {
                    Some(index) => index,
                    None => return concrete::BindingId::from_raw(local.binding.into_raw()),
                }
            }
            export::DefaultCaptureSourceV1::EnclosingCapture(index) => *index as usize,
        };
        let export::ImportedCallableTemplateOrigin::Local { parent, descriptor } =
            &template.declaration
        else {
            panic!("an enclosing capture belongs to a lexical implementation")
        };
        self.imported_capture_binding(*parent, descriptor.captures()[capture_index].source())
    }
}
