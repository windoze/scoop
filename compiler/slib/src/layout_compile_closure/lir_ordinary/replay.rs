use scoop_hir as hir;
use scoop_identity::{CallableTemplateOrigin, DependencyCallableDeclarationId, GcEffect};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::super::lir_callable_layouts::Layouts;
use super::SharedOrdinaryLirBridgeValidationError as Error;

mod layouts;
mod selected;

#[derive(Clone, Copy)]
pub struct SharedOrdinaryLirBridgeDependenciesV1<'a> {
    pub metadata: &'a [hir::SharedTypeMetadataV1<'a>],
    pub layouts: &'a [&'a lir::CanonicalExactLayoutExportsV1],
    pub callables: &'a [&'a lir::CrossConeLirBridgeSectionV1],
}

/// Replays the existing ordinary bridge from HIR-joined MIR declarations and
/// uses. Dependencies must be limited to the current artifact's reachable
/// providers; candidate LIR records are not inputs to this projection.
pub fn replay_shared_ordinary_lir_bridge(
    target: lir::LirTargetProfile,
    source: hir::SharedTypeMetadataV1<'_>,
    mir: &mir::CrossConeMirBridgeSectionV1,
    local: &lir::CanonicalExactLayoutExportsV1,
    dependencies: SharedOrdinaryLirBridgeDependenciesV1<'_>,
    foundation: &lir::OdrFreeLirFoundation,
) -> Result<lir::CrossConeLirBridgeSectionV1, Error> {
    if mir.artifact() != foundation.producer() || source.provider != foundation.producer() {
        return Err(Error::ArtifactProvider);
    }
    let layouts = Layouts::new(local, dependencies.layouts, target, foundation.producer())?;
    let path = WirePath::root();
    let selected = selected::replay(mir, dependencies)?;
    let mut sources = Vec::new();
    scoop_wire::allocation::try_reserve(&mut sources, dependencies.metadata.len(), &path)?;
    sources.extend(
        dependencies
            .metadata
            .iter()
            .copied()
            .map(crate::AbiReplayDependency::from),
    );
    let types = crate::collect_abi_types(source.into(), &sources)?;

    let mut exports = Vec::new();
    scoop_wire::allocation::try_reserve(&mut exports, mir.exports().len(), &path)?;
    for callable in mir.exports() {
        let declaration = callable.declaration();
        let origin = match declaration {
            DependencyCallableDeclarationId::Function(id) => CallableTemplateOrigin::Function(id),
            DependencyCallableDeclarationId::PropertyAccessor(id) => {
                CallableTemplateOrigin::Accessor(id)
            }
        };

        let interface = source
            .public
            .callable_interfaces()
            .get(origin)
            .ok_or(Error::CallableInterface(declaration))?;
        let gc = interface.effects().gc_effect();
        let signature = types.replay(target, callable.signature(), gc)?;
        layouts::check(declaration, &signature, target, &layouts)?;
        let root = match gc {
            GcEffect::Managed => lir::ExternalCallableRootPlan::ManagedStatepoint,
            GcEffect::NoGc => lir::ExternalCallableRootPlan::NoGc,
        };
        let export = lir::ParamFreeLirCallableExportV1::new(
            foundation.producer(),
            declaration,
            callable.implementation(),
            signature,
            lir::CallingConvention::Cdecl,
            root,
        )
        .map_err(|source| Error::Export {
            declaration,
            source: Box::new(source),
        })?;
        exports.push(export);
    }
    Ok(lir::CrossConeLirBridgeSectionV1::try_new(
        foundation, exports, selected,
    )?)
}
