//! Inline type-parameter dependencies over shared value definitions.

use super::*;

impl Lowerer {
    pub(super) fn value_layout_parameter_relevance(
        &self,
        nodes: &[ValueLayoutTemplate],
    ) -> HashMap<hir::SourceNominalId, HashSet<hir::TypeParamId>> {
        let mut definitions = nodes
            .iter()
            .map(|&node| {
                let owner = match node {
                    ValueLayoutTemplate::Struct(id) => Owner::Struct(id),
                    ValueLayoutTemplate::Enum(id) => Owner::Enum(id),
                };
                (
                    self.nominal_identity(owner).declaration_id(),
                    self.value_layout_field_types(node),
                )
            })
            .collect::<Vec<_>>();
        // Only parameter relevance crosses dependencies. SCC vertices remain
        // current declarations; each provider checked its own declaration graph.
        definitions.extend(
            self.loaded_struct_definitions
                .iter()
                .map(|(&owner, value)| {
                    (
                        owner,
                        value
                            .definition
                            .semantic_fields()
                            .iter()
                            .map(|field| field.ty)
                            .collect(),
                    )
                }),
        );
        definitions.extend(self.loaded_enum_definitions.iter().map(|(&owner, value)| {
            (
                owner,
                value
                    .definition
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|field| field.ty))
                    .collect(),
            )
        }));
        let mut relevance = definitions
            .iter()
            .map(|(owner, _)| (*owner, HashSet::new()))
            .collect::<HashMap<_, _>>();

        loop {
            let mut changed = false;
            for (owner, fields) in &definitions {
                let mut discovered = HashSet::new();
                for &ty in fields {
                    self.collect_inline_type_parameters(ty, &relevance, &mut discovered);
                }
                let known = relevance
                    .get_mut(owner)
                    .expect("every value template has a relevance entry");
                let previous_len = known.len();
                known.extend(discovered);
                changed |= known.len() != previous_len;
            }
            if !changed {
                return relevance;
            }
        }
    }

    fn collect_inline_type_parameters(
        &self,
        ty: hir::TypeId,
        relevance: &HashMap<hir::SourceNominalId, HashSet<hir::TypeParamId>>,
        out: &mut HashSet<hir::TypeParamId>,
    ) {
        match &self.types[ty] {
            Type::Param(parameter) => {
                out.insert(*parameter);
            }
            Type::Tuple(elements) => {
                for &element in elements {
                    self.collect_inline_type_parameters(element, relevance, out);
                }
            }
            Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                let Some(relevant) = relevance.get(&application.template) else {
                    return;
                };
                for (parameter, &argument) in self
                    .struct_definition(application.template)
                    .type_params
                    .iter()
                    .zip(&application.arguments)
                {
                    if relevant.contains(&parameter.id) {
                        self.collect_inline_type_parameters(argument, relevance, out);
                    }
                }
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                let Some(relevant) = relevance.get(&application.template) else {
                    return;
                };
                for (parameter, &argument) in self
                    .enum_definition(application.template)
                    .type_params
                    .iter()
                    .zip(&application.arguments)
                {
                    if relevant.contains(&parameter.id) {
                        self.collect_inline_type_parameters(argument, relevance, out);
                    }
                }
            }
            // Every item below is a scalar or an indirection boundary. In
            // particular, pointer pointees and reference type arguments are
            // not stored inline and therefore do not make parameters layout
            // relevant.
            Type::Unit
            | Type::Integer(_)
            | Type::Boolean
            | Type::String
            | Type::Class(_)
            | Type::Interface(_)
            | Type::Any
            | Type::Function(_)
            | Type::Ptr(_)
            | Type::FunPtr(_) => {}
        }
    }
}
