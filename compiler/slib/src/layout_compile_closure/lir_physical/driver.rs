use super::*;

type PhysicalInput<'input> = LirConstituentsValidatedCrossConeLayoutSections<
    'input,
    lir::PhysicalImportsReplayedLayoutAbiSectionV1,
    lir::CrossConeLirBridgeSectionV1,
    lir::CompileProduction,
    mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
>;

impl<'input> LirDependenciesResolvedCrossConeLayoutClosure<'input> {
    pub(super) fn read_physical<O>(
        self,
        mut read_data: impl FnMut(&mut PhysicalInput<'input>) -> Result<O, SharedLirPhysicalError>,
    ) -> Result<
        (PhysicalImportsReplayedCrossConeLayoutClosure, Vec<O>),
        CrossConeLayoutLirPhysicalError,
    > {
        let mut complete = Vec::new();
        let mut objects = Vec::new();
        for (position, artifact) in self.dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let read = || -> Result<_, SharedLirPhysicalError> {
                let LirDependenciesResolvedCrossConeLayoutSections {
                    mut prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let parts = prepared.semantic_parts();
                let reachable = transitive_positions(position, &self.dependency_positions)?;
                let mut dependencies = Vec::new();
                scoop_wire::allocation::try_reserve(
                    &mut dependencies,
                    reachable.len(),
                    &WirePath::root(),
                )?;
                dependencies.extend(reachable.iter().map(|&index| &complete[index]));
                let layout = read::physical(layout, &dependencies, parts.identities)?;

                scoop_wire::allocation::try_reserve(&mut complete, 1, &WirePath::root())?;
                scoop_wire::allocation::try_reserve_count(&mut objects, 1, &WirePath::root())?;
                let mut artifact = PhysicalInput {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                };
                let object = read_data(&mut artifact)?;
                let PhysicalInput {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                } = artifact;
                let artifact = PhysicalImportsReplayedCrossConeLayoutSections {
                    semantic: prepared.into_semantics(),
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                };
                Ok((artifact, object))
            };
            let (artifact, object) = read().map_err(|source| CrossConeLayoutLirPhysicalError {
                provider,
                source: Box::new(source),
            })?;
            complete.push(artifact);
            objects.push(object);
        }
        Ok((
            PhysicalImportsReplayedCrossConeLayoutClosure {
                current: self.current,
                target: self.target,
                direct: self.direct,
                artifacts: complete,
                positions: self.positions,
            },
            objects,
        ))
    }
}
