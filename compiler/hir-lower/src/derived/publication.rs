//! Exporting a closed public value type requests its conditional equality body.

use super::*;

impl Lowerer {
    pub(crate) fn prepare_public_derived_equalities(&mut self) {
        let owners = self
            .structs
            .iter()
            .filter_map(|(id, value)| {
                (value.type_params.is_empty()
                    && value.access.declared == hir::DeclaredVisibility::Public
                    && value.access.lookup.0.is_universal()
                    && value.derived_equality.is_some())
                .then_some((Owner::Struct(id), value.span, self.struct_files[&id]))
            })
            .chain(self.enums.iter().filter_map(|(id, value)| {
                (value.type_params.is_empty()
                    && value.access.declared == hir::DeclaredVisibility::Public
                    && value.access.lookup.0.is_universal()
                    && value.derived_equality.is_some())
                .then_some((Owner::Enum(id), value.span, self.enum_files[&id]))
            }))
            .collect::<Vec<_>>();
        for (owner, span, file) in owners {
            let mut candidate = self.clone();
            let previous = candidate.current_file;
            candidate.current_file = file;
            let ty = candidate.owner_ty(owner);
            if candidate.derived_equality_candidate(ty, span).is_ok() {
                candidate.current_file = previous;
                *self = candidate;
            }
        }
    }
}
