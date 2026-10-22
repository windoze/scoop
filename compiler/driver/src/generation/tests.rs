use super::*;
use hir::SemanticQueries;
use scoop_identity::{NormalizedSourcePath, SourceIdentity};
use std::path::Path;

mod assertions;

struct Inspect {
    checked: bool,
    slots: Vec<(
        hir::InterfaceMethodId,
        scoop_identity::PersistentDispatchSlotId,
    )>,
}

impl hir::MethodBodyGenerator for Inspect {
    fn generate(
        &mut self,
        queries: &mut dyn SemanticQueries,
    ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<Diagnostic>> {
        let annotations = configuration::annotations(queries).unwrap();
        let valid = select(
            &annotations,
            &[
                "merge",
                "multi",
                "partial",
                "handA",
                "handB",
                "empty",
                "default",
                "inherited",
                "generic",
            ],
        );
        let batches = plan::plan(queries, valid).unwrap();
        assertions::positive(queries, &batches);
        assertions::errors(queries, &annotations);
        for method in queries.pending_methods() {
            for requirement in method.requirements {
                if let hir::InterfaceMethodReference::Local(method) = requirement.declaration {
                    self.slots.push((
                        method,
                        queries.requirement_slot(requirement.declaration).unwrap(),
                    ));
                }
            }
        }
        self.checked = true;
        Ok(queries
            .pending_methods()
            .iter()
            .map(|method| generated(queries, method.function))
            .collect())
    }
}

fn select(annotations: &[GenerationAnnotation], generators: &[&str]) -> Vec<GenerationAnnotation> {
    annotations
        .iter()
        .filter(|annotation| {
            let hir::CanonicalAnnotationValueV1::Scalar(hir::CanonicalConstValueV1::String(
                generator,
            )) = &annotation.application.arguments[0]
            else {
                panic!("ordinary bound generator string")
            };
            generators.contains(&generator.as_str())
        })
        .cloned()
        .collect()
}

fn generated(
    queries: &mut dyn SemanticQueries,
    function: hir::FunctionId,
) -> hir::GeneratedMethodBody {
    let method = MethodId::from(&queries.method_identity(function).unwrap());
    let id = match method {
        MethodId::Function(id) => id.to_string(),
        MethodId::GenericFunction(id) => id.to_string(),
    };
    let path = format!("generated/plans/{id}.scoop");
    hir::GeneratedMethodBody {
        function,
        body: scoop_parser::parse_function_body("{ return 1 }").unwrap(),
        source: hir::GeneratedMethodSource {
            identity: SourceIdentity::new(
                queries.current_cone(),
                NormalizedSourcePath::new(&path).unwrap(),
            )
            .unwrap(),
            name: path,
            text: "{ return 1 }".to_owned().into(),
        },
        origin: queries.signature(function).origin,
    }
}

fn source_input(
    source: &scoop_manifest::DiscoveredSource,
) -> scoop_parser::CurrentConeSourceInput<'_> {
    scoop_parser::CurrentConeSourceInput::new(
        source.identity(),
        source.source_text(),
        source.display_locator().as_path(),
    )
    .with_role(source.role())
}

#[test]
fn generator_plans_use_real_core_annotations_and_existing_conformance_bindings() {
    let manifest =
        scoop_toolchain::load_cone_manifest(&scoop_manifest::ManifestRootLocator::cone_directory(
            crate::workspace_root().join("sysroot/src/scoop.core"),
        ))
        .unwrap();
    let sources =
        scoop_manifest::discover_manifest_sources(&manifest, scoop_target_data::DARWIN_AARCH64_ID)
            .unwrap();
    let identity = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/generator-plans.scoop").unwrap(),
    )
    .unwrap();
    let mut inputs = sources.iter().map(source_input).collect::<Vec<_>>();
    inputs.push(scoop_parser::CurrentConeSourceInput::new(
        &identity,
        include_str!("tests/source.scoop"),
        Path::new("generator-plans.scoop"),
    ));
    inputs.sort_by(|left, right| left.identity().cmp(right.identity()));
    let mut inputs = inputs.into_iter();
    let parsed = scoop_parser::parse_current_cone(NonEmptyVec::new(
        inputs.next().unwrap(),
        inputs.collect(),
    ))
    .unwrap();
    let world =
        hir::ImportedSemanticWorld::from_dependencies(ConeIdentity::CORE, Vec::new(), Vec::new())
            .unwrap();
    let input = scoop_hir_lower::CurrentConeSources::try_new(
        &parsed,
        scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap();
    let mut inspect = Inspect {
        checked: false,
        slots: Vec::new(),
    };
    let output = scoop_hir_lower::lower_current_cone_with_generator(
        scoop_identity::RequestedConeKind::Library,
        &input,
        &mut inspect,
    )
    .unwrap();
    assert!(inspect.checked);
    for (method, slot) in inspect.slots {
        assert_eq!(
            output.output().export.dispatch_slot_identities[method].id(),
            slot
        );
    }
}
