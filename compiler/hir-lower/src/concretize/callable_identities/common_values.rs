//! Shared initialization keeps captured values in their original lexical scope.

use std::collections::{BTreeMap, HashSet};

use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn materialize_common_initialization_values(
        &mut self,
    ) -> Vec<concrete::LexicalLocalValueScope> {
        let mut captured = HashSet::new();
        for function in &self.concretizer.function_slots {
            let function = function
                .as_ref()
                .expect("requested functions have complete bodies");
            captured.extend(
                function
                    .capture_parameters
                    .iter()
                    .map(|capture| capture.binding),
            );
        }
        for captures in self
            .concretizer
            .lambdas
            .values()
            .map(|lambda| &lambda.captures)
            .chain(
                self.concretizer
                    .anonymous_functions
                    .values()
                    .map(|function| &function.captures),
            )
            .chain(
                self.concretizer
                    .callable_reference_slots
                    .iter()
                    .map(|reference| &reference.captures),
            )
        {
            captured.extend(captures.iter().map(|capture| capture.binding));
        }
        let mut scopes: BTreeMap<
            CallableMaterialization,
            Vec<concrete::LexicalLocalValueDefinition>,
        > = BTreeMap::new();
        let mut seen = HashSet::new();
        let mut classes = HashSet::new();
        for &(_, class) in &self.concretizer.class_constructor_definitions {
            if !classes.insert(class) {
                continue;
            }
            let origin = self.concretizer.classes[class].origin.declaration_id();
            let initializers = class_common_initializations(self.concretizer.source, origin);
            let declarations = self.class_constructor_declarations(class);
            let arguments = self.concretizer.classes[class].type_arguments.clone();
            for step in initializers.into_iter().flatten() {
                let locals = match step {
                    export::ClassInitializationStep::Field { initializer, .. } => {
                        &initializer.locals
                    }
                    export::ClassInitializationStep::InitBlock { body, .. } => &body.locals,
                };
                for local in locals.values() {
                    let binding = concrete::BindingId::from_raw(local.binding.into_raw());
                    if !captured.contains(&binding) {
                        continue;
                    }
                    let export::LocalValueDefinitionSite::Source(origin) = local.definition else {
                        continue;
                    };
                    let source_context = self
                        .concretizer
                        .source
                        .source_context_identities
                        .get(origin.context)
                        .expect("a source local retains its definition context");
                    let scoop_identity::SourceContextKey::Callable {
                        owner: scoop_identity::CallableOwner::Constructor(constructor),
                        ..
                    } = source_context.key()
                    else {
                        continue;
                    };
                    if !declarations.contains(constructor) {
                        continue;
                    }
                    let owner = self.constructor_materialization(
                        CallableTemplateOwner::Constructor(*constructor),
                        *constructor,
                        self.concretizer.class_type[&class],
                        &arguments,
                    );
                    if seen.insert((owner, binding)) {
                        scopes.entry(owner).or_default().push(
                            concrete::LexicalLocalValueDefinition {
                                binding,
                                selector: local.selector.clone(),
                                definition: local.definition,
                            },
                        );
                    }
                }
            }
        }
        scopes
            .into_iter()
            .map(|(owner, values)| concrete::LexicalLocalValueScope { owner, values })
            .collect()
    }

    pub(super) fn class_constructor_declarations(
        &self,
        class: concrete::ClassId,
    ) -> HashSet<scoop_identity::PersistentConstructorId> {
        let source = self.concretizer.source;
        let origin = self.concretizer.classes[class].origin.declaration_id();
        match source.nominal_identities.class_id(origin) {
            Some(class) => source.classes[class]
                .constructors
                .iter()
                .filter_map(|constructor| {
                    source.constructor_identities[*constructor]
                        .source_record()
                        .map(|record| record.id())
                })
                .collect(),
            None => source.loaded_class_definitions[&origin]
                .declaration
                .interface
                .declaration_details()
                .constructors()
                .values()
                .iter()
                .copied()
                .collect(),
        }
    }
}

fn class_common_initializations(
    source: &export::Module,
    origin: export::SourceNominalId,
) -> Vec<&[export::ClassInitializationStep]> {
    fn common(kind: &export::ClassConstructorKind) -> Option<&[export::ClassInitializationStep]> {
        match kind {
            export::ClassConstructorKind::Primary {
                common_initialization,
                ..
            }
            | export::ClassConstructorKind::Secondary {
                delegation:
                    export::ClassSecondaryDelegation::Terminal {
                        common_initialization,
                        ..
                    },
                ..
            } => Some(common_initialization.as_slice()),
            export::ClassConstructorKind::Secondary {
                delegation: export::ClassSecondaryDelegation::This { .. },
                ..
            } => None,
        }
    }
    match source.nominal_identities.class_id(origin) {
        Some(class) => source.classes[class]
            .constructors
            .iter()
            .filter_map(|id| common(&source.class_constructors[*id].kind))
            .collect(),
        None => source
            .imported_constructor_templates
            .values()
            .filter_map(|template| {
                let export::Type::Class(owner) = source.types[template.owner] else {
                    return None;
                };
                if source.class_applications[owner].template != origin {
                    return None;
                }
                let export::ConstructorKind::Class(kind) = &template.kind else {
                    unreachable!("a class constructor retains its class body");
                };
                common(kind)
            })
            .collect(),
    }
}
