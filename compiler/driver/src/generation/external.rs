use super::*;
use scoop_generator_protocol::Generated;
use scoop_toolchain::GeneratorPrograms;

mod diagnostics;
mod fragments;
mod session;

pub(crate) struct ExternalGenerator<'a> {
    programs: &'a GeneratorPrograms,
    pub sources: Vec<hir::GeneratedMethodSource>,
    pub warnings: Vec<Diagnostic>,
    pub used: Vec<scoop_protocol::GeneratorToolInputV1>,
}

impl<'a> ExternalGenerator<'a> {
    pub fn new(programs: &'a GeneratorPrograms) -> Self {
        Self {
            programs,
            sources: Vec::new(),
            warnings: Vec::new(),
            used: Vec::new(),
        }
    }
}

impl hir::MethodBodyGenerator for ExternalGenerator<'_> {
    fn generate(
        &mut self,
        queries: &mut dyn hir::SemanticQueries,
    ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<Diagnostic>> {
        let batches = plan_generators(queries)?;
        let mut fragments = Vec::new();
        let mut errors = Vec::new();
        for batch in batches {
            let origin = &batch
                .tasks
                .first()
                .expect("planned batches contain tasks")
                .methods
                .first()
                .origin;
            let program = self
                .programs
                .program(&batch.generator)
                .map_err(|error| vec![diagnostic(origin, error.to_string())])?
                .ok_or_else(|| {
                    vec![diagnostic(
                        origin,
                        format!(
                            "no host program is registered for generator `{}`",
                            batch.generator
                        ),
                    )]
                })?;
            let used = scoop_toolchain::record_generator_input(
                &batch.generator,
                &program,
                scoop_generator_protocol::PROTOCOL_VERSION,
            )
            .map_err(|error| vec![diagnostic(origin, error.to_string())])?;
            let Generated {
                bodies,
                diagnostics,
            } = session::generate(queries, &batch, &program).map_err(|error| vec![error])?;
            self.used.push(used);
            for reported in diagnostics {
                match diagnostics::resolve(queries, reported, origin) {
                    Ok(diagnostic)
                        if diagnostic.severity == scoop_ast::DiagnosticSeverity::Warning =>
                    {
                        self.warnings.push(diagnostic);
                    }
                    Ok(diagnostic) | Err(diagnostic) => errors.push(diagnostic),
                }
            }
            fragments.extend(fragments::collect(
                queries.current_cone(),
                &batch,
                bodies,
                &mut self.sources,
                &mut errors,
            ));
            if !errors.is_empty() {
                return Err(errors);
            }
        }
        // Every query observes the original fixed frontend view, before any body is parsed/lowered.
        fragments::parse(fragments)
    }
}
