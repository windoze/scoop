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
                let reachable =
                    transitive_positions(position, &self.dependency_positions, parts.meter)?;
                let mut dependencies = Vec::new();
                parts.meter.try_reserve_collection_slots(
                    &mut dependencies,
                    reachable.len(),
                    &WirePath::root(),
                )?;
                dependencies.extend(reachable.iter().map(|&index| complete[index]));
                let layout = replay::physical(
                    layout,
                    &strong,
                    &mir,
                    &dependencies,
                    parts.identities,
                    parts.meter,
                )?;
                strong.validate_replayed_layout_selection(&layout, parts.meter)?;
                if let Some(link) = parts.link_sections {
                    link.layout_link_closure_wire()
                        .validate_physical_imports_against(
                            layout.physical_imports(),
                            parts.meter,
                        )?;
                }
                parts.meter.charge_owned_bytes(
                    std::mem::size_of::<PhysicalImportsReplayedCrossConeLayoutSections<'_, '_>>()
                        as u64,
                    &WirePath::root(),
                )?;
                parts
                    .meter
                    .try_reserve_collection_slots(&mut complete, 1, &WirePath::root())?;
                parts.meter.try_reserve_exact(
                    &mut objects,
                    1,
                    std::mem::size_of::<O>() as u64,
                    &WirePath::root(),
                )?;
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
