use scoop_identity::ConeIdentity;
use scoop_slib::{self as slib, CrossConeLayoutSemanticClosureError as Error, SlibDiagnostic};

pub(super) fn location(error: &Error) -> Option<(ConeIdentity, String)> {
    let (provider, path) = match error {
        Error::Graph(error) => return graph(error),
        Error::Identity(error) => match error.as_ref() {
            slib::CrossConeClosureIdentityError::Artifact { identity, source } => {
                return Some((*identity, source.diagnostic().semantic_path()));
            }
            slib::CrossConeClosureIdentityError::AuthorityAllocation { identity, .. } => {
                (*identity, "identity")
            }
            slib::CrossConeClosureIdentityError::Allocation { .. } => return None,
        },
        Error::Foundation(error) => match error.as_ref() {
            slib::CrossConeClosureFoundationError::Artifact { identity, .. }
            | slib::CrossConeClosureFoundationError::SourceProvenance { identity, .. } => {
                (*identity, "foundation")
            }
            slib::CrossConeClosureFoundationError::Allocation { .. } => return None,
        },
        Error::HirResolution(error) => match error.as_ref() {
            slib::CrossConeLayoutClosureHirResolutionError::Artifact { identity, .. } => {
                (*identity, "hir")
            }
            slib::CrossConeLayoutClosureHirResolutionError::Allocation { .. } => return None,
        },
        Error::HirProduction(error) => match error.as_ref() {
            slib::CrossConeClosureHirProductionError::Artifact { identity, .. } => {
                (*identity, "hir")
            }
            slib::CrossConeClosureHirProductionError::Allocation { .. } => return None,
        },
        Error::HirDeclarations(error) => (error.provider, "hir/declarations"),
        Error::SourceCallables(error) => (error.provider, "mir/source-callables"),
        Error::LirLayouts(error) => (error.provider, "lir/layouts"),
        Error::LirCallableAbis(error) => (error.provider, "lir/callable-abis"),
        Error::LirDispatch(error) => (error.provider, "lir/dispatch"),
        Error::LirDescriptors(error) => (error.provider, "lir/descriptors"),
        Error::LirShapeSupport(error) => (error.provider, "lir/shape-support"),
        Error::OrdinaryLirBridges(error) => (error.provider, "lir/ordinary-bridges"),
        Error::MirDependencies(error) => (error.provider, "mir/dependencies"),
        Error::LirDependencies(error) => (error.provider, "lir/dependencies"),
        Error::LirStrongProduction(error) => return Some((error.provider, strong(&error.source))),
    };
    Some((provider, path.to_owned()))
}

fn strong(error: &slib::SharedLirStrongProductionError) -> String {
    use scoop_lir::{
        ConeProductionSectionValidationError as Production,
        StrongRegistrationProductionValidationError as Registration,
    };
    use slib::SharedLirStrongProductionError as Strong;
    let capability = slib::lir_cone_production_capability();
    let prefix = format!("lir/{}/{}", capability.name(), capability.major_version());
    match error {
        Strong::InitializationRelation { unit, field } => {
            format!("{prefix}/initialization/{unit}/{field}")
        }
        Strong::InitializationDefinition(unit) => format!("{prefix}/initialization/{unit}"),
        Strong::FunctionDescriptor { exact, field } => {
            format!("{prefix}/function-descriptor/{exact}/{field}")
        }
        Strong::Replay(Production::Registrations(Registration::TableLength { table, .. })) => {
            format!("{prefix}/registrations/{table:?}")
        }
        Strong::Replay(Production::Registrations(_)) => format!("{prefix}/registrations"),
        _ => prefix,
    }
}

fn graph(error: &slib::CrossConeClosureGraphError) -> Option<(ConeIdentity, String)> {
    use slib::CrossConeClosureGraphError as Graph;
    let identity = match error {
        Graph::DuplicateArtifact { identity }
        | Graph::InvalidProviderKind { identity, .. }
        | Graph::TargetMismatch { identity, .. }
        | Graph::UnreachableSupport { identity } => *identity,
        Graph::MissingDependencyArtifact { dependent, .. }
        | Graph::InvalidDependencyFirstOrder { dependent, .. }
        | Graph::StaleDependency { dependent, .. } => *dependent,
        Graph::MultipleVersions { second, .. } => second.identity().ok()?,
        Graph::NonCanonicalDirectProviders { .. }
        | Graph::CurrentArtifactPresent { .. }
        | Graph::CurrentArtifactIdentityMismatch { .. }
        | Graph::CurrentDirectSetMismatch { .. }
        | Graph::MissingDirectArtifact { .. } => return None,
    };
    Some((identity, "manifest".to_owned()))
}
