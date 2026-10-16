//! Type metadata becomes ordinary values and calls in concrete HIR.

use super::*;

mod annotation_keys;
mod annotations;
mod constructors;
mod fields;
mod owners;
mod properties;
pub(super) use owners::CallableBody;
mod references;
mod shapes;
mod source_types;
mod type_uses;
mod values;

impl Concretizer<'_> {
    pub(super) fn type_info_origin(
        &self,
        key: &FunctionKey,
        span: scoop_ast::Span,
    ) -> export::ConcreteExpressionOrigin {
        let definition = match self.function_source(key) {
            FunctionSource::Imported(id) => self.source.imported_generic_templates[id].origin,
            FunctionSource::Local(id) => {
                let (context, source) = self
                    .source
                    .source_contexts
                    .iter()
                    .find(|(_, context)| {
                        context.subject() == &export::SourceContextSubject::Function(id)
                    })
                    .expect("a metadata intrinsic retains its source context");
                let (file, source) = self
                    .source
                    .source_files
                    .iter()
                    .enumerate()
                    .find(|(_, file)| &file.identity == source.source())
                    .expect("a source context retains its file");
                export::DefinitionOrigin {
                    provider: source.provider,
                    file: file as u32,
                    span,
                    context,
                }
            }
            FunctionSource::AnnotationDescription { origin, .. } => origin.definition,
            FunctionSource::Companion(..) => {
                unreachable!("metadata intrinsics are top-level functions")
            }
        };
        export::ConcreteExpressionOrigin {
            definition,
            evaluation: definition.into(),
        }
    }

    fn type_reference_factory(&self) -> FunctionSource {
        match self.core {
            export::CoreProtocols::Defined(core) => FunctionSource::Local(core.type_reference),
            export::CoreProtocols::Imported(core) => {
                let export::ImportedCoreProtocolCallableDefinition::GenericFunction(factory) =
                    core.type_reference().definition()
                else {
                    unreachable!("the core reference factory is generic")
                };
                let (id, _) = self
                    .source
                    .imported_generic_templates
                    .iter()
                    .find(|(_, template)| {
                        template.declaration.body_owner()
                            == export::DefaultCallableDeclarationV1::GenericFunction(
                                factory.persistent(),
                            )
                    })
                    .expect("the source typeInfo body retains its ordinary reference factory");
                FunctionSource::Imported(id)
            }
        }
    }

    fn type_reference_value(
        &mut self,
        target: concrete::TypeId,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let source = self.type_reference_factory();
        let key = self.function_key(source, None, vec![target]);
        let function = self.request_function_key(key, source);
        value(
            concrete::ExprKind::Call {
                callee: concrete::CallableTarget::Local(concrete::Callable::Function(function)),
                args: Vec::new(),
                receiver: export::SourceCallReceiver::NoReceiver,
                binding: None,
            },
            result,
            origin,
        )
    }
}

fn value(
    kind: concrete::ExprKind,
    ty: concrete::TypeId,
    origin: export::ConcreteExpressionOrigin,
) -> concrete::Expr {
    concrete::Expr {
        kind,
        ty,
        span: origin.definition.span,
        origin,
    }
}

fn nominal_identity(nominal: export::SourceNominalId) -> String {
    match nominal {
        export::SourceNominalId::Concrete(id) => format!("type:{id}"),
        export::SourceNominalId::GenericTemplate(id) => format!("generic-type:{id}"),
    }
}
