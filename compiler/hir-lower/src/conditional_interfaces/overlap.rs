use super::*;

impl Lowerer {
    pub(super) fn check_conditional_interface_overlap(
        &mut self,
        owner: Owner,
        declaration: &hir::ConditionalInterfaceDeclaration,
    ) {
        let ordinary = match owner {
            Owner::Class(id) => self.classes[id].interfaces.clone(),
            Owner::Object(id) => self.classes[self.objects[id].backing_class]
                .interfaces
                .clone(),
            Owner::Struct(id) => self.structs[id].interfaces.clone(),
            Owner::Enum(id) => self.enums[id].interfaces.clone(),
            Owner::Interface(_) => {
                unreachable!("conditional implementations require ordinary owners")
            }
        };
        for interface in ordinary {
            if self.open_types_may_equal(interface, declaration.interface) {
                self.error(declaration.span, format!(
                    "conditional implementation of `{}` may overlap the unconditional interface `{}` on {}",
                    self.type_name(declaration.interface), self.type_name(interface), owner.describe(self),
                ));
            }
        }
        let previous = self.owner_conditional_interfaces(owner).to_vec();
        for previous in previous {
            if self.open_types_may_equal(previous.interface, declaration.interface)
                && !self.conditional_environments_disjoint(
                    &previous.parameters,
                    &declaration.parameters,
                )
            {
                let mut diagnostic = ast::Diagnostic::at_file(
                    self.current_file,
                    declaration.span,
                    format!(
                        "interface implementations of `{}` and `{}` on {} may overlap; a stronger condition does not override another implementation",
                        self.type_name(previous.interface),
                        self.type_name(declaration.interface),
                        owner.describe(self),
                    ),
                );
                diagnostic.notes.push(ast::DiagnosticNote::at(
                    self.current_file,
                    previous.span,
                    "the other implementation is here",
                ));
                self.diagnostics.push(diagnostic);
            }
        }
    }

    fn conditional_environments_disjoint(
        &mut self,
        left: &[hir::TypeParamDecl],
        right: &[hir::TypeParamDecl],
    ) -> bool {
        left.iter().zip(right).any(|(left, right)| {
            matches!(
                (left.kind(), right.kind()),
                (hir::TypeParamKind::Value, hir::TypeParamKind::Ref)
                    | (hir::TypeParamKind::Ref, hir::TypeParamKind::Value)
            ) || self.nominal_bounds_disjoint(&left.bounds, &right.bounds)
                || left
                    .companion
                    .as_ref()
                    .zip(right.companion.as_ref())
                    .is_some_and(|(left, right)| {
                        self.nominal_bounds_disjoint(&left.bounds, &right.bounds)
                    })
        })
    }

    fn nominal_bounds_disjoint(
        &mut self,
        left: &hir::TypeParamBounds,
        right: &hir::TypeParamBounds,
    ) -> bool {
        let (hir::TypeParamBounds::Nominal(left), hir::TypeParamBounds::Nominal(right)) =
            (left, right)
        else {
            return false;
        };
        let (Some(left), Some(right)) = (&left.class, &right.class) else {
            return false;
        };
        // Closed class applications have one inheritance chain. Open arguments
        // require more information, so they do not establish disjointness.
        self.closed_class_bounds_are_disjoint(left.ty, right.ty)
    }
}
