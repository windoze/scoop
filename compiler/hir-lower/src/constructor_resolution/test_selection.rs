use super::*;

impl Lowerer {
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
