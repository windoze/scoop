use super::*;
use hir::{AnnotationCallableV1 as Callable, SourceAnnotationCallable as SourceCallable};
use scoop_identity::CallableTemplateOrigin as Declaration;

impl DeclarationReferences {
    pub(super) fn callables(&mut self, lowerer: &Lowerer, data: &body_projection::ProjectionData) {
        for (function, identity) in data.functions.iter() {
            let Some((reference, source)) = function_reference(function, identity, data) else {
                continue;
            };
            let file = lowerer
                .function_definition_file(function)
                .expect("source callable has its definition file");
            let origin = hir::SemanticSourceLocation {
                source: lowerer.source_identity(file).clone(),
                span: lowerer.functions[function].span,
            };
            self.body_parameters(lowerer, function, reference, source, &origin);
            if matches!(identity, hir::HirFunctionIdentity::PropertyAccessor(_)) {
                continue;
            }
            self.current(
                Target::Callable(reference),
                Source::Callable(source),
                origin.clone(),
            );
            if lowerer.functions[function].method.is_some()
                || lowerer.extension_receivers.contains_key(&function)
            {
                self.current(
                    Target::CallableReceiver(reference),
                    Source::Receiver(hir::SourceAnnotationReceiver::Callable(source)),
                    origin,
                );
            }
        }
        for (id, declaration) in lowerer.struct_constructors.iter() {
            self.current(
                Target::Callable(Callable::Declaration(Declaration::Constructor(
                    data.constructors[id].id(),
                ))),
                Source::Callable(SourceCallable::StructConstructor(id)),
                source_location(lowerer, declaration.origin),
            );
        }
        for (id, declaration) in lowerer.class_constructors.iter() {
            if let Some(record) = data.constructors[id].source_record() {
                self.current(
                    Target::Callable(Callable::Declaration(Declaration::Constructor(record.id()))),
                    Source::Callable(SourceCallable::ClassConstructor(id)),
                    source_location(lowerer, declaration.origin),
                );
            }
        }
        for interface in &lowerer.source_parameter_interfaces {
            let Some((reference, source)) = parameter_owner(lowerer, data, interface.owner) else {
                continue;
            };
            for (index, parameter) in interface.parameters.iter().enumerate() {
                let index = index as u32;
                self.current(
                    Target::Parameter {
                        callable: reference,
                        index,
                    },
                    Source::Parameter {
                        callable: source,
                        index,
                    },
                    source_location(lowerer, parameter.origin),
                );
            }
        }
    }

    fn body_parameters(
        &mut self,
        lowerer: &Lowerer,
        function: hir::FunctionId,
        callable: Callable,
        source: SourceCallable,
        origin: &hir::SemanticSourceLocation,
    ) {
        let declaration = &lowerer.functions[function];
        let hir::FunctionKind::User(body) = &declaration.kind else {
            return;
        };
        for parameter in &declaration.params {
            let local = &body.locals[parameter.local];
            match local.selector {
                scoop_identity::LocalValueSelector::Parameter {
                    declaration_index: index,
                } => {
                    let location = match local.definition {
                        hir::LocalValueDefinitionSite::Source(origin) => {
                            source_location(lowerer, origin)
                        }
                        hir::LocalValueDefinitionSite::Synthetic => origin.clone(),
                    };
                    self.current(
                        Target::Parameter { callable, index },
                        Source::Parameter {
                            callable: source,
                            index,
                        },
                        location,
                    );
                }
                scoop_identity::LocalValueSelector::This => self.current(
                    Target::CallableReceiver(callable),
                    Source::Receiver(hir::SourceAnnotationReceiver::Callable(source)),
                    origin.clone(),
                ),
                _ => {}
            }
        }
    }
}

fn function_reference(
    function: hir::FunctionId,
    identity: &hir::HirFunctionIdentity,
    data: &body_projection::ProjectionData,
) -> Option<(Callable, SourceCallable)> {
    let (declaration, source) = match identity {
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => (
            Declaration::Function(record.id()),
            SourceCallable::Function(function),
        ),
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => (
            Declaration::GenericFunction(record.id()),
            SourceCallable::Function(function),
        ),
        hir::HirFunctionIdentity::LexicalGenerated(record) => {
            return Some((
                Callable::Local(record.id()),
                SourceCallable::Function(function),
            ));
        }
        hir::HirFunctionIdentity::PropertyAccessor(hir::HirPropertyAccessorFunction::Getter(
            id,
        )) => (
            Declaration::Accessor(data.accessors[*id].id()),
            SourceCallable::Getter(*id),
        ),
        hir::HirFunctionIdentity::PropertyAccessor(hir::HirPropertyAccessorFunction::Setter(
            id,
        )) => (
            Declaration::Accessor(data.accessors[*id].id()),
            SourceCallable::Setter(*id),
        ),
        hir::HirFunctionIdentity::TestEntry(_)
        | hir::HirFunctionIdentity::Initialization { .. }
        | hir::HirFunctionIdentity::DerivedEquality(_) => return None,
    };
    Some((Callable::Declaration(declaration), source))
}

fn parameter_owner(
    lowerer: &Lowerer,
    data: &body_projection::ProjectionData,
    owner: hir::ExportParameterOwner,
) -> Option<(Callable, SourceCallable)> {
    let (declaration, source) = match owner {
        hir::ExportParameterOwner::Function(id) => {
            return function_reference(id, &data.functions[id], data);
        }
        hir::ExportParameterOwner::StructConstructor(id) => (
            Declaration::Constructor(data.constructors[id].id()),
            SourceCallable::StructConstructor(id),
        ),
        hir::ExportParameterOwner::ClassConstructor(id) => (
            Declaration::Constructor(data.constructors[id].source_record()?.id()),
            SourceCallable::ClassConstructor(id),
        ),
        hir::ExportParameterOwner::VariantConstructor(id) => (
            Declaration::VariantConstructor(
                lowerer
                    .enum_member_identities
                    .as_ref()
                    .expect("variant identity is established")[id]
                    .id(),
            ),
            SourceCallable::VariantConstructor(id),
        ),
    };
    Some((Callable::Declaration(declaration), source))
}
