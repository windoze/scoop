use super::*;

pub(super) fn plan(
    queries: &mut dyn hir::SemanticQueries,
    annotations: Vec<GenerationAnnotation>,
) -> Result<Vec<GeneratorBatch>, Vec<Diagnostic>> {
    let pending = queries.pending_methods();
    let mut planner = Planner::default();
    let mut diagnostics = Vec::new();
    for annotation in annotations {
        let origin = queries.source_location(annotation.application.definition_origin);
        if matches!(annotation.owner, hir::NominalOwner::Interface(_)) {
            diagnostics.push(diagnostic(
                &origin,
                "GenerateWith must be applied to an implementation type, not an interface",
            ));
            continue;
        }
        let (generator, elements) =
            match configuration::parameters(&annotation.application, &origin) {
                Ok(parameters) => parameters,
                Err(error) => {
                    diagnostics.push(error);
                    continue;
                }
            };
        let existing = queries.interfaces(annotation.owner);
        for element in elements {
            let binding =
                match configuration::interface(queries, annotation.owner, element, &existing) {
                    Ok(binding) => binding,
                    Err(errors) => {
                        diagnostics.extend(errors);
                        continue;
                    }
                };
            if let Err(error) =
                planner.bind(queries, &pending, annotation.owner, generator, binding)
            {
                diagnostics.push(error);
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(planner
        .batches
        .into_iter()
        .filter_map(|batch| {
            let tasks = batch
                .tasks
                .into_iter()
                .filter_map(|task| {
                    let mut methods = task.methods.into_iter();
                    let first = methods.next()?;
                    let mut bindings = task.bindings.into_iter();
                    Some(GeneratorTask {
                        owner: task.owner,
                        bindings: NonEmptyVec::new(
                            bindings.next().expect("a task is created from a binding"),
                            bindings.collect(),
                        ),
                        methods: NonEmptyVec::new(first, methods.collect()),
                    })
                })
                .collect::<Vec<_>>();
            (!tasks.is_empty()).then_some(GeneratorBatch {
                generator: batch.generator,
                tasks,
            })
        })
        .collect())
}

#[derive(Default)]
struct Planner {
    bindings: HashMap<(hir::NominalOwner, hir::TypeId), hir::SemanticSourceLocation>,
    methods: HashMap<hir::FunctionId, (String, hir::SemanticSourceLocation)>,
    batches: Vec<BatchDraft>,
}

struct BatchDraft {
    generator: String,
    tasks: Vec<TaskDraft>,
}

struct TaskDraft {
    owner: hir::NominalOwner,
    bindings: Vec<GeneratorBinding>,
    methods: Vec<GeneratorMethod>,
}

impl Planner {
    fn bind(
        &mut self,
        queries: &mut dyn hir::SemanticQueries,
        pending: &[hir::PendingMethod],
        owner: hir::NominalOwner,
        generator: &str,
        binding: GeneratorBinding,
    ) -> Result<(), Diagnostic> {
        if let Some(previous) = self.bindings.get(&(owner, binding.interface)) {
            return Err(diagnostic(
                &binding.origin,
                "duplicate GenerateWith interface application",
            )
            .with_note(related(previous, "previous interface binding is here")));
        }
        self.bindings
            .insert((owner, binding.interface), binding.origin.clone());
        let requirements = queries.requirements(binding.interface);
        let mut methods = Vec::new();
        for method in pending.iter().filter(|method| {
            method.owner == owner
                && method
                    .requirements
                    .iter()
                    .any(|requirement| requirements.contains(requirement))
        }) {
            if let Some((previous_generator, previous_origin)) = self.methods.get(&method.function)
            {
                if previous_generator != generator {
                    return Err(diagnostic(&binding.origin, format!(
                        "generators `{previous_generator}` and `{generator}` select the same missing method"))
                        .with_note(related(previous_origin, "previous method binding is here")));
                }
                continue;
            }
            let identity = queries.method_identity(method.function)?;
            self.methods.insert(
                method.function,
                (generator.to_owned(), binding.origin.clone()),
            );
            methods.push(GeneratorMethod {
                method: MethodId::from(&identity),
                pending: method.clone(),
                origin: binding.origin.clone(),
            });
        }
        let index = match self
            .batches
            .iter()
            .position(|batch| batch.generator == generator)
        {
            Some(index) => index,
            None => {
                self.batches.push(BatchDraft {
                    generator: generator.to_owned(),
                    tasks: Vec::new(),
                });
                self.batches.len() - 1
            }
        };
        let batch = &mut self.batches[index];
        if let Some(task) = batch.tasks.iter_mut().find(|task| task.owner == owner) {
            task.bindings.push(binding);
            task.methods.extend(methods);
        } else {
            batch.tasks.push(TaskDraft {
                owner,
                bindings: vec![binding],
                methods,
            });
        }
        Ok(())
    }
}
