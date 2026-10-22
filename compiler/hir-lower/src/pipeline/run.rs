use super::*;

#[cfg(test)]
mod generation_tests;
#[cfg(test)]
mod phase_tests;

impl Lowerer {
    #[cfg(test)]
    pub(crate) fn run_defined(
        self,
        files: &[ast::SourceFile],
    ) -> Result<(hir::Module, Vec<Diagnostic>), Vec<Diagnostic>> {
        let dependencies = hir::ImportedDependencySelectionPlan::empty(self.current_cone());
        let (module, warnings, _) = self
            .with_imported_dependencies(dependencies)
            .run(files, None, None, &cancellation::uninterrupted)
            .map_err(PhaseFailure::into_diagnostics)?;
        Ok((module, warnings))
    }

    pub(crate) fn run_with_dependencies<C>(
        self,
        files: &[ast::SourceFile],
        world: &hir::ImportedSemanticWorld<'_>,
        generator: Option<&mut dyn hir::MethodBodyGenerator>,
        check: &impl Fn() -> Result<(), C>,
    ) -> Result<(hir::Module, Vec<Diagnostic>, LoweringCompletion), PhaseFailure<C>> {
        self.run(files, Some(world), generator, check)
    }

    fn run<C>(
        mut self,
        files: &[ast::SourceFile],
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        generator: Option<&mut dyn hir::MethodBodyGenerator>,
        check: &impl Fn() -> Result<(), C>,
    ) -> Result<(hir::Module, Vec<Diagnostic>, LoweringCompletion), PhaseFailure<C>> {
        self.generation.enabled = generator.is_some();
        let declarations = self.collect_declaration_inputs_with_cancellation(
            files.iter().map(ast::SourceFile::header).collect(),
            files.iter().enumerate().flat_map(|(file, source)| {
                source
                    .declarations
                    .iter()
                    .map(move |declaration| (file, ast::DeclarationInput::Complete(declaration)))
            }),
            world,
            check,
        )?;
        let current_cone = declarations.current_cone;
        let SignatureCore {
            intrinsic_type_core,
            source_location_core,
            iteration_core,
            coroutine_core,
            ffi_core,
            foreign_callback_core,
        } = self.resolve_source_signatures_with_cancellation(&declarations, check)?;
        PhaseFailure::check(check)?;
        let DeclarationBodyCore {
            program_arguments,
            type_reference,
            exception_core,
        } = self.complete_declaration_bodies(&declarations)?;
        self.lower_source_bodies_with_cancellation(&declarations, type_reference, check)?;
        PhaseFailure::check(check)?;
        if let Some(generator) = generator {
            self.generate_method_bodies(generator, world);
        }
        PhaseFailure::check(check)?;
        if self.diagnostics.is_empty() {
            self.discover_tests();
        }
        if self.diagnostics.is_empty() && self.test_compilation.is_some() {
            self.generate_test_entry();
        }
        if self.diagnostics.is_empty()
            && let Err(error) = self.prepare_coroutine_declarations()
        {
            self.error(Span::new(0, 0), error);
        }
        self.complete_imported_generic_bodies();
        PhaseFailure::check(check)?;
        if self.diagnostics.is_empty()
            && let Err(error) = self.prepare_type_info_dependencies()
        {
            self.error(Span::new(0, 0), error);
        }
        self.complete_imported_generic_bodies();
        PhaseFailure::check(check)?;
        if self.diagnostics.is_empty() {
            self.prepare_published_equalities();
        }

        // Effects consume fully resolved calls and types. Local functions and
        // callable literals lifted while lowering the bodies are visible now.
        self.validate_c_ffi_types();
        PhaseFailure::check(check)?;
        self.check_generic_recursion();
        PhaseFailure::check(check)?;
        let type_sites = self.validate_gc_free_pointee_requirements();
        self.check_no_gc_types(type_sites.clone());
        self.check_no_gc_functions();
        PhaseFailure::check(check)?;
        if self.diagnostics.is_empty() {
            let release_values = self.infer_release_callability();
            self.check_release_blocks(&release_values, &type_sites);
        }

        self.warnings.sort_by_key(|diagnostic| {
            let span = diagnostic.span.unwrap_or(Span {
                start: u32::MAX,
                end: u32::MAX,
            });
            (diagnostic.file, span.start, span.end)
        });
        if !self.diagnostics.is_empty() {
            self.diagnostics.append(&mut self.warnings);
            return Err(self.take_source_diagnostics().into());
        }
        let warnings = std::mem::take(&mut self.warnings);
        // Protocol provenance and dependency selections are independent.
        let core_protocols = match self.core.clone() {
            CoreLoweringAuthority::Defined => {
                hir::CoreProtocols::Defined(Box::new(hir::DefinedCoreProtocols {
                    program_arguments: program_arguments
                        .expect("an invalid program argument builder is diagnosed"),
                    type_reference: type_reference
                        .expect("an invalid type reference factory is diagnosed"),
                    option: self
                        .option_core
                        .expect("a missing or invalid core `Option` is always diagnosed"),
                    iteration: iteration_core
                        .expect("a missing or invalid core iteration protocol is always diagnosed"),
                    exceptions: exception_core
                        .expect("a missing or invalid compiler exception core is always diagnosed"),
                    coroutines: coroutine_core
                        .expect("a missing or invalid coroutine core protocol is always diagnosed"),
                    ffi: ffi_core
                        .expect("a missing or invalid FFI core protocol is always diagnosed"),
                    foreign_callbacks: foreign_callback_core.expect(
                        "a missing or invalid foreign callback core protocol is always diagnosed",
                    ),
                    fundamental_types: intrinsic_type_core
                        .expect("missing or invalid intrinsic core types are always diagnosed"),
                    source_location: source_location_core
                        .expect("a missing or invalid source location core is always diagnosed"),
                }))
            }
            CoreLoweringAuthority::Imported(authority) => hir::CoreProtocols::Imported(authority),
        };
        let sources = self.diagnostic_source_identities();
        let documentation = self.finish_documentation(world);
        PhaseFailure::check(check)?;
        self.finish(current_cone, warnings, core_protocols, documentation)
            .map_err(|mut diagnostics| {
                for diagnostic in &mut diagnostics {
                    diagnostic.resolve_sources(&sources);
                }
                diagnostics.into()
            })
    }
}
