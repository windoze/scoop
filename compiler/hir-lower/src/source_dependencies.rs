//! Project complete source HIR into the same declarations used by artifact inputs.

use std::{fmt, rc::Rc};

use scoop_hir as hir;
use scoop_identity::{PendingIdentityValidation, SemanticIdentitySession};

pub fn project_source_dependency(
    output: &hir::DependencyExportHirOutput,
    world: &hir::ImportedSemanticWorld<'_>,
    session: &mut SemanticIdentitySession,
) -> Result<hir::OwnedImportedProvider, SourceDependencyProjectionError> {
    let native = crate::persistent_native_boundary::build_source(&output.export, world)
        .map_err(|error| failure("native types", error))?;
    let mut foundation = hir::CanonicalHirFoundation::from_source_export(&output.export, &native)
        .map_err(|error| failure("source identities", error))?;
    let mut authority = hir::CrossConeHirProductionAuthority::new(
        &foundation,
        &output.export.public_export_bindings,
        world,
    );
    let interface = hir::CrossConeHirInterfaceSectionV1::from_source_hir(output, &mut authority)
        .map_err(|error| failure("declarations", error))?;
    foundation
        .complete_cross_cone_interface_source_points(&output.export, &interface)
        .map_err(|error| failure("source locations", error))?;
    let aliases = interface
        .type_aliases()
        .expand_alias_closure(
            &world.alias_expansions().collect::<Vec<_>>(),
            &scoop_wire::WirePath::root(),
        )
        .map_err(|error| failure("type aliases", error))?;
    let core = hir::CoreBootstrapInterfaceSectionV1::from_export(&output.export)
        .map_err(|error| failure("compiler protocols", error))?;
    let mut identities = PendingIdentityValidation::new();
    foundation
        .register_identities(&mut identities)
        .map_err(|error| failure("source identity graph", error))?;
    let identities = identities
        .finish()
        .map_err(|error| failure("source identity graph", error))?;
    let identities = session
        .import_source(output.export.cone, &identities)
        .map_err(|error| failure("semantic identities", error))?
        .into_parts()
        .0;
    Ok(hir::OwnedImportedProvider {
        foundation: Rc::new(hir::ImportedHirFoundation::from_shared(
            Rc::new(foundation),
            identities,
        )),
        interface: Rc::new(interface),
        alias_expansions: Rc::new(aliases),
        core: Rc::new(core),
    })
}

#[derive(Debug)]
pub struct SourceDependencyProjectionError(String);

fn failure(part: &str, error: impl fmt::Display) -> SourceDependencyProjectionError {
    SourceDependencyProjectionError(format!("cannot project source dependency {part}: {error}"))
}

impl fmt::Display for SourceDependencyProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SourceDependencyProjectionError {}
