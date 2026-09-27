use super::*;

type PhysicalInput<'input> = LirConstituentsValidatedCrossConeLayoutSections<
    'input,
    lir::PhysicalImportsReplayedLayoutAbiSectionV1,
    lir::CrossConeLirBridgeSectionV1,
    lir::ConeProductionSectionV2,
    mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
>;

impl<'input> LirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub(super) fn replay_physical<O>(
        self,
        mut replay_objects: impl FnMut(
            &mut PhysicalInput<'input>,
            &[usize],
            &[O],
            &[PhysicalImportsReplayedCrossConeLayoutSections],
        ) -> Result<O, SharedLirPhysicalError>,
    ) -> Result<
        (PhysicalImportsReplayedCrossConeLayoutClosure, Vec<O>),
        CrossConeLayoutLirPhysicalError,
    > {
        let mut complete = Vec::new();
        let mut objects = Vec::new();
        for (position, artifact) in self.dependency_first.into_iter().enumerate() {
            let provider = artifact.identity();
            let replay = || -> Result<_, SharedLirPhysicalError> {
                let LirDependencyGraphReplayedCrossConeLayoutSections {
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
                let layout = replay::physical(layout, &dependencies, parts.identities)?;
                strong.validate_layout_selection(&layout)?;
                if let Some(link) = parts.link_sections {
                    link.layout_link_closure_wire()
                        .validate_physical_imports_against(layout.physical_imports())?;
                }

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
                let object = replay_objects(&mut artifact, &reachable, &objects, &complete)?;
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
            let (artifact, object) =
                replay().map_err(|source| CrossConeLayoutLirPhysicalError {
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
