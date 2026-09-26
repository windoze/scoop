//! Definition-time validation of finite inline value layouts.
//!
//! A concrete application cannot repair a declaration cycle that embeds a
//! struct or enum back into itself by value. Generic arguments need slightly
//! more care: an argument contributes to layout only when the target
//! declaration uses that parameter inline. Computing that relevance first
//! keeps phantom parameters and every reference boundary out of the graph,
//! while still exposing cycles carried through generic wrappers.

use std::collections::{HashMap, HashSet};

use scoop_hir as hir;

use crate::{Lowerer, Owner, Type, ValueLayoutTemplate};

use self::graph::{find_component_cycle, strongly_connected_components};

mod graph;

impl Lowerer {
    /// Reject every strongly connected component in the declaration-level
    /// by-value dependency graph. The graph is finite even for a declaration
    /// such as `Grow<T> -> Grow<(T, T)>`: applications never have to be
    /// materialized to recognize that its template returns to itself.
    pub(crate) fn validate_value_layout_cycles(&mut self) -> bool {
        let nodes = self.value_layout_templates();
        let relevance = self.value_layout_parameter_relevance(&nodes);
        let mut graph = HashMap::with_capacity(nodes.len());
        for &node in &nodes {
            let mut dependencies = Vec::new();
            let mut seen = HashSet::new();
            for ty in self.value_layout_field_types(node) {
                self.collect_value_layout_dependencies(
                    ty,
                    &relevance,
                    &mut dependencies,
                    &mut seen,
                );
            }
            graph.insert(node, dependencies);
        }

        let order = nodes
            .iter()
            .enumerate()
            .map(|(index, &node)| (node, index))
            .collect::<HashMap<_, _>>();
        for dependencies in graph.values_mut() {
            dependencies.sort_by_key(|node| order[node]);
        }

        let mut components = strongly_connected_components(&nodes, &graph);
        components.sort_by_key(|component| {
            component
                .iter()
                .map(|node| order[node])
                .min()
                .expect("a strongly connected component is non-empty")
        });
        let mut diagnostics = Vec::new();
        let mut valid = true;
        for mut component in components {
            component.sort_by_key(|node| order[node]);
            let cyclic = component.len() > 1
                || graph[&component[0]]
                    .iter()
                    .any(|target| *target == component[0]);
            if !cyclic {
                continue;
            }

            valid = false;
            let cycle = find_component_cycle(component[0], &component, &graph);
            let path = cycle
                .iter()
                .map(|node| self.value_layout_template_name(*node))
                .collect::<Vec<_>>()
                .join(" -> ");
            let root = cycle[0];
            diagnostics.push((
                self.value_layout_template_file(root),
                self.value_layout_template_span(root),
                format!("recursive value layout does not cross a reference boundary: {path}"),
            ));
        }

        let previous_file = self.current_file;
        for (file, span, message) in diagnostics {
            self.current_file = file;
            self.error(span, message);
        }
        self.current_file = previous_file;
        valid
    }

    fn value_layout_templates(&self) -> Vec<ValueLayoutTemplate> {
        let mut nodes = self
            .structs
            .iter()
            .filter_map(|(id, declaration)| {
                matches!(
                    declaration.representation,
                    hir::StructRepresentation::Declared(_)
                )
                .then_some(ValueLayoutTemplate::Struct(id))
            })
            .chain(
                self.enums
                    .iter()
                    .map(|(id, _)| ValueLayoutTemplate::Enum(id)),
            )
            .collect::<Vec<_>>();
        nodes.sort_by_key(|node| self.value_layout_template_order(*node));
        nodes
    }

    fn value_layout_parameter_relevance(
        &self,
        nodes: &[ValueLayoutTemplate],
    ) -> HashMap<ValueLayoutTemplate, HashSet<hir::TypeParamId>> {
        let mut relevance = nodes
            .iter()
            .copied()
            .map(|node| (node, HashSet::new()))
            .collect::<HashMap<_, _>>();

        loop {
            let mut changed = false;
            for &node in nodes {
                let mut discovered = HashSet::new();
                for ty in self.value_layout_field_types(node) {
                    self.collect_inline_type_parameters(ty, &relevance, &mut discovered);
                }
                let known = relevance
                    .get_mut(&node)
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
        relevance: &HashMap<ValueLayoutTemplate, HashSet<hir::TypeParamId>>,
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
                let target = ValueLayoutTemplate::Struct(application.template);
                let Some(relevant) = relevance.get(&target) else {
                    return;
                };
                for (parameter, &argument) in self.structs[application.template]
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
                let target = ValueLayoutTemplate::Enum(application.template);
                let relevant = &relevance[&target];
                for (parameter, &argument) in self.enums[application.template]
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
            Type::ImportedStruct(_)
            | Type::ImportedEnum(_)
            | Type::Unit
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

    fn collect_value_layout_dependencies(
        &self,
        ty: hir::TypeId,
        relevance: &HashMap<ValueLayoutTemplate, HashSet<hir::TypeParamId>>,
        out: &mut Vec<ValueLayoutTemplate>,
        seen: &mut HashSet<ValueLayoutTemplate>,
    ) {
        match &self.types[ty] {
            Type::Tuple(elements) => {
                for &element in elements {
                    self.collect_value_layout_dependencies(element, relevance, out, seen);
                }
            }
            Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                let target = ValueLayoutTemplate::Struct(application.template);
                let Some(relevant) = relevance.get(&target) else {
                    return;
                };
                if seen.insert(target) {
                    out.push(target);
                }
                for (parameter, &argument) in self.structs[application.template]
                    .type_params
                    .iter()
                    .zip(&application.arguments)
                {
                    if relevant.contains(&parameter.id) {
                        self.collect_value_layout_dependencies(argument, relevance, out, seen);
                    }
                }
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                let target = ValueLayoutTemplate::Enum(application.template);
                if seen.insert(target) {
                    out.push(target);
                }
                let relevant = &relevance[&target];
                for (parameter, &argument) in self.enums[application.template]
                    .type_params
                    .iter()
                    .zip(&application.arguments)
                {
                    if relevant.contains(&parameter.id) {
                        self.collect_value_layout_dependencies(argument, relevance, out, seen);
                    }
                }
            }
            Type::ImportedStruct(_)
            | Type::ImportedEnum(_)
            | Type::Unit
            | Type::Integer(_)
            | Type::Boolean
            | Type::String
            | Type::Class(_)
            | Type::Interface(_)
            | Type::Any
            | Type::Function(_)
            | Type::Ptr(_)
            | Type::FunPtr(_)
            | Type::Param(_) => {}
        }
    }

    fn value_layout_field_types(&self, node: ValueLayoutTemplate) -> Vec<hir::TypeId> {
        match node {
            ValueLayoutTemplate::Struct(id) => self.structs[id]
                .semantic_fields()
                .iter()
                .map(|field| field.ty)
                .collect(),
            ValueLayoutTemplate::Enum(id) => self.enums[id]
                .variants
                .iter()
                .flat_map(|variant| variant.fields.iter().map(|field| field.ty))
                .collect(),
        }
    }

    fn value_layout_template_order(&self, node: ValueLayoutTemplate) -> (usize, u32, u8, u32) {
        let (file, span, kind, raw) = match node {
            ValueLayoutTemplate::Struct(id) => (
                self.struct_files[&id],
                self.structs[id].span,
                0,
                id.into_raw().into_u32(),
            ),
            ValueLayoutTemplate::Enum(id) => (
                self.enum_files[&id],
                self.enums[id].span,
                1,
                id.into_raw().into_u32(),
            ),
        };
        (file, span.start, kind, raw)
    }

    fn value_layout_template_file(&self, node: ValueLayoutTemplate) -> usize {
        match node {
            ValueLayoutTemplate::Struct(id) => self.struct_files[&id],
            ValueLayoutTemplate::Enum(id) => self.enum_files[&id],
        }
    }

    fn value_layout_template_span(&self, node: ValueLayoutTemplate) -> scoop_ast::Span {
        match node {
            ValueLayoutTemplate::Struct(id) => self.structs[id].span,
            ValueLayoutTemplate::Enum(id) => self.enums[id].span,
        }
    }

    fn value_layout_template_name(&self, node: ValueLayoutTemplate) -> String {
        match node {
            ValueLayoutTemplate::Struct(id) => Owner::Struct(id).describe_name(self),
            ValueLayoutTemplate::Enum(id) => Owner::Enum(id).describe_name(self),
        }
    }
}
