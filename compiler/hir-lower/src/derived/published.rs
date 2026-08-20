use super::*;

impl Lowerer {
    pub(crate) fn prepare_published_equalities(&mut self) {
        let owners = self
            .structs
            .iter()
            .filter_map(|(id, declaration)| {
                declaration
                    .type_params
                    .is_empty()
                    .then_some((Owner::Struct(id), declaration.derived_equality))
            })
            .chain(self.enums.iter().filter_map(|(id, declaration)| {
                declaration
                    .type_params
                    .is_empty()
                    .then_some((Owner::Enum(id), declaration.derived_equality))
            }))
            .filter_map(|(owner, function)| function.map(|function| (owner, function)))
            .collect::<Vec<_>>();
        for (owner, function) in owners {
            if !Self::declaration_is_exported(&self.functions[function].access) {
                continue;
            }
            let mut candidate = self.clone();
            let previous_file = candidate.current_file;
            let previous_owner = candidate.current_owner.replace(owner);
            candidate.current_file = candidate.function_files[&function];
            let ty = candidate.owner_ty(owner);
            let span = candidate.functions[function].span;
            if matches!(candidate.derived_equality_candidate(ty, span), Ok(Some(_))) {
                candidate.current_file = previous_file;
                candidate.current_owner = previous_owner;
                *self = candidate;
            }
        }
    }
}
