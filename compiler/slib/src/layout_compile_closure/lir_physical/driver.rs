use super::*;

impl<'input> LirDependencyGraphReplayedCrossConeLayoutClosure<'input> {
    pub(super) fn with_replayed_physical<R, O>(
        self,
        mut replay_objects: impl FnMut(
            &mut PhysicalImportsReplayedCrossConeLayoutSections<'input, '_>,
            &[usize],
            &[O],
            &[&PhysicalImportsReplayedCrossConeLayoutSections<'input, '_>],
        ) -> Result<O, SharedLirPhysicalError>,
        use_checked: impl for<'checked> FnOnce(
            PhysicalImportsReplayedCrossConeLayoutClosure<'checked, 'input>,
            Vec<O>,
        ) -> R,
    ) -> Result<R, CrossConeLayoutLirPhysicalError> {
        let arena = Arena::new();
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
                dependencies.extend(reachable.iter().map(|&index| complete[index]));
                let layout =
                    replay::physical(layout, &strong, &mir, &dependencies, parts.identities)?;
                strong.validate_layout_selection(&layout)?;
                if let Some(link) = parts.link_sections {
                    link.layout_link_closure_wire()
                        .validate_physical_imports_against(layout.physical_imports())?;
                }

                scoop_wire::allocation::try_reserve(&mut complete, 1, &WirePath::root())?;
                scoop_wire::allocation::try_reserve_count(&mut objects, 1, &WirePath::root())?;
                let mut artifact = PhysicalImportsReplayedCrossConeLayoutSections {
                    prepared,
                    mir,
                    units,
                    strong,
                    ordinary,
                    layout,
                };
                let object = replay_objects(&mut artifact, &reachable, &objects, &complete)?;
                Ok((artifact, object))
            };
            let (artifact, object) =
                replay().map_err(|source| CrossConeLayoutLirPhysicalError {
                    provider,
                    source: Box::new(source),
                })?;
            complete.push(&*arena.alloc(artifact));
            objects.push(object);
        }
        Ok(use_checked(
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
