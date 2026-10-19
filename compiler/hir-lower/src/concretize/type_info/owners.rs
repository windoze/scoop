use super::*;
use scoop_identity::CallableTemplateOwner;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::concretize) enum CallableBody {
    Function(concrete::FunctionId),
    ClassConstructor(concrete::ClassConstructorId),
    StructConstructor(concrete::StructConstructorId),
}

impl Concretizer<'_> {
    pub(in crate::concretize) fn metadata_parent_request(
        &self,
        parent: CallableBody,
    ) -> (
        CallableTemplateOwner,
        Option<concrete::MethodOwner>,
        &[concrete::TypeId],
    ) {
        match parent {
            CallableBody::Function(id) => {
                let key = &self.function_keys[id.into_raw().into_u32() as usize];
                (
                    key.template_owner()
                        .expect("a metadata query has a source callable"),
                    key.owner,
                    &key.arguments,
                )
            }
            CallableBody::ClassConstructor(id) => {
                let (definition, owner) =
                    self.class_constructor_definitions[id.into_raw().into_u32() as usize];
                let template = match self.class_constructor_origin(definition).0 {
                    export::DefaultClassConstructorIdV1::Source(id) => {
                        CallableTemplateOwner::Constructor(id)
                    }
                    export::DefaultClassConstructorIdV1::Generated(id) => {
                        CallableTemplateOwner::Generated(id)
                    }
                };
                (
                    template,
                    Some(concrete::MethodOwner::Class(owner)),
                    &self.classes[owner].type_arguments,
                )
            }
            CallableBody::StructConstructor(id) => {
                let (definition, owner) =
                    self.struct_constructor_definitions[id.into_raw().into_u32() as usize];
                (
                    CallableTemplateOwner::Constructor(self.struct_constructor_origin(definition)),
                    Some(concrete::MethodOwner::Struct(owner)),
                    &self.structs[owner].type_arguments,
                )
            }
        }
    }
}
