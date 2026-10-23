use super::*;

impl Lowerer {
    pub(super) fn annotate_interface_blocks<'a>(
        &mut self,
        owner: Owner,
        ordinary_count: usize,
        blocks: impl Iterator<Item = &'a ast::InterfaceImplementationDecl>,
    ) {
        let previous = self.type_params_in_scope.clone();
        for (index, block) in blocks.enumerate() {
            let Some(declaration) = self
                .owner_conditional_interfaces(owner)
                .iter()
                .find(|declaration| declaration.span == block.span)
            else {
                continue;
            };
            self.type_params_in_scope = declaration.parameters.clone();
            self.annotate_type_site(
                hir::SourceAnnotationTarget::Nominal(owner.as_nominal_owner()),
                hir::AnnotationTypeSlot::Supertype((ordinary_count + index) as u32),
                &block.interface,
            );
            for property in block.properties() {
                self.annotate_property_declaration(property, self.current_file);
            }
        }
        self.type_params_in_scope = previous;
    }
}
