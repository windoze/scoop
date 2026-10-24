use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_inline_bodies(&mut self) -> Arena<concrete::ImportedInlineBody> {
        let mut bodies = Arena::new();
        for (_, template) in self.source.imported_generic_templates.iter() {
            let (declaration, owner) = match template.declaration {
                export::ImportedCallableTemplateOrigin::Inline(owner) => (
                    scoop_identity::CallableTemplateOrigin::Function(owner),
                    scoop_identity::CallableTemplateOwner::Function(owner),
                ),
                export::ImportedCallableTemplateOrigin::InlineAccessor(owner) => (
                    scoop_identity::CallableTemplateOrigin::Accessor(owner),
                    scoop_identity::CallableTemplateOwner::Accessor(owner),
                ),
                _ => continue,
            };
            if !export::small_inline_body(&template.signature.signature, &template.implementation) {
                continue;
            }
            let Some((callable, _)) =
                self.imported_dependency_callables.iter().find(|(_, use_)| {
                    use_.reference().declaration() == declaration
                        && use_.dispatch() == export::ImportedDependencyDispatch::Direct
                })
            else {
                continue;
            };
            let export::FunctionKind::User(body) = &template.implementation else {
                unreachable!()
            };
            let (body, locals) = self.lower_body(body, &[]);
            let params = template
                .params
                .iter()
                .map(|parameter| concrete::Param {
                    name: parameter.name.clone(),
                    ty: self.lower_type(parameter.ty, &[]),
                    local: locals[parameter.local.into_raw().into_u32() as usize],
                })
                .collect();
            let return_ty = self.lower_type(template.return_ty, &[]);
            bodies.alloc(concrete::ImportedInlineBody {
                callable,
                materialization: scoop_identity::CallableMaterialization::new(
                    owner,
                    scoop_identity::CallableMaterializationContext::NoSubstitution,
                ),
                params,
                return_ty,
                body,
                span: template.span,
            });
        }
        if !bodies.is_empty() {
            self.drain_pending_callables();
        }
        bodies
    }
}
