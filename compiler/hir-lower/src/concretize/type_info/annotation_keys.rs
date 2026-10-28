use super::*;
use scoop_identity::{
    PersistentAnnotationId, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

impl Concretizer<'_> {
    pub(in crate::concretize) fn metadata_annotation_key(
        &mut self,
        annotation: PersistentAnnotationId,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let identity = self.metadata_string(format!("annotation:{annotation}"), origin);
        let declaration = self
            .source
            .static_annotation_declaration(annotation, self.dependencies);
        let scoop_identity::DeclarationName::Named(name) = declaration.key().name() else {
            unreachable!("annotation declarations are named")
        };
        let name = self.metadata_string(name.as_str().to_owned(), origin);
        let concrete::TypeKind::Function(function_type) = self.types[fields[2]].kind else {
            unreachable!("AnnotationKey.describeBody is an ordinary closure")
        };
        let description = self.function_types[function_type].return_type;
        let current = self
            .current_body
            .expect("metadata is generated in a callable body");
        let parent = match current {
            CallableBody::Function(id) => {
                match self.function_sources[id.into_raw().into_u32() as usize] {
                    FunctionSource::AnnotationDescription { parent, .. } => parent,
                    _ => current,
                }
            }
            _ => current,
        };
        let (_, owner, arguments) = self.metadata_parent_request(parent);
        let arguments = arguments.to_vec();
        let source = FunctionSource::AnnotationDescription {
            annotation,
            parent,
            result: description,
            origin,
        };
        let key = self.function_key(source, owner, arguments);
        let function = self.request_function_key(key, source);
        let lambda = self.lambdas.alloc(concrete::Lambda {
            definition_path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
                [],
            ),
            function,
            function_type,
            captures: Vec::new(),
            span: origin.definition.span,
        });
        let describe = value(concrete::ExprKind::Lambda(lambda), fields[2], origin);
        self.metadata_record(result, vec![identity, name, describe], origin)
    }

    pub(in crate::concretize) fn lower_annotation_description(
        &mut self,
        annotation: PersistentAnnotationId,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> functions::PendingFunction {
        let fields = self.metadata_fields(result);
        let declaration = self
            .source
            .static_annotation_declaration(annotation, self.dependencies);
        let key = self.metadata_annotation_key(annotation, fields[0], origin);
        let visibility = self.metadata_visibility(fields[1], declaration.visibility(), origin);
        let parameters = self.metadata_annotation_parameters(declaration, fields[2], origin);
        let annotations = self.metadata_annotations(
            declaration.annotations(self.source).iter(),
            fields[3],
            origin,
        );
        let description = self.metadata_record(
            result,
            vec![key, visibility, parameters, annotations],
            origin,
        );
        functions::PendingFunction {
            debug_source: None,
            name: format!("<annotation-description:{annotation}>"),
            is_suspend: false,
            modifiers: export::CallableModifiers::default(),
            params: Vec::new(),
            capture_parameters: Vec::new(),
            return_ty: result,
            attributes: export::FunctionAttributes::default(),
            kind: concrete::FunctionKind::User(concrete::Body {
                locals: Arena::new(),
                statements: vec![concrete::Statement {
                    kind: concrete::StatementKind::Return { value: description },
                    span: origin.definition.span,
                }],
            }),
            receiver: concrete::FunctionReceiver::None,
            span: origin.definition.span,
        }
    }
}
