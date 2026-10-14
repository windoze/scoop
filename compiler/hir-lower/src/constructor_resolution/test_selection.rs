use super::*;

impl Lowerer {
    pub(crate) fn construct_test_instance(
        &mut self,
        container: &hir::TestContainer,
        span: ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let source = NominalConstructorSource::Class(container.constructor);
        let view = self.nominal_constructor_view(source, span);
        let argument_map = CandidateArgumentMap::source_nominal(&view, &[])
            .expect("test discovery selected this zero-argument constructor");
        let args = self.materialize_nominal_arguments(
            crate::argument_materialization::NominalArgumentMaterialization {
                receiver: None,
                view: &view,
                argument_map: &argument_map,
                type_args: &[],
                source_args: Vec::new(),
                argument_sinks: Vec::new(),
                call_span: span,
            },
            sink,
        )?;
        let application = self.class_application_id(container.class, Vec::new());
        let constructor = self.class_constructor_application(container.constructor, application);
        Some(hir::Expr {
            kind: hir::ExprKind::ClassInit { constructor, args },
            ty: self.class_applications[application].canonical_type,
            span,
            origin: self.expression_origin(span),
        })
    }

    /// Selects the actual zero-argument invocation for a checked test class.
    /// The existing default templates remain on the selected constructor.
    pub(crate) fn select_test_constructor(
        &mut self,
        class: hir::ClassId,
        span: ast::Span,
    ) -> Option<hir::ClassConstructorId> {
        let name = self.classes[class].name.clone();
        let candidates = self
            .source_class_constructors(class)
            .map(NominalConstructorSource::Class)
            .collect::<Vec<_>>();
        let outer_file = std::mem::replace(&mut self.current_file, self.class_files[&class]);
        let outer_owner = self.current_owner.replace(crate::Owner::Class(class));
        self.push_safety_context(hir::Safety::Safe);
        let selected = self.select_nominal_constructor(
            &name,
            &candidates,
            NominalConstructorCall {
                enclosing: None,
                explicit_type_args: &[],
                expected_type_args: None,
                arguments: &[],
                span,
            },
        );
        let selected = selected.and_then(|winner| {
            *self = *winner.state;
            let before = self.diagnostics.len();
            self.check_constructor_call_safety(winner.source, span);
            let NominalConstructorSource::Class(constructor) = winner.source else {
                unreachable!("class selection has only class constructor candidates")
            };
            (self.diagnostics.len() == before).then_some(constructor)
        });
        self.pop_safety_context();
        self.current_owner = outer_owner;
        self.current_file = outer_file;
        selected
    }
}
