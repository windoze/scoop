//! Shared interning of complete source and artifact identity inputs.

use super::*;

impl SemanticIdentitySession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn origin_count(&self) -> usize {
        self.origins.len()
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn origin_fingerprint(&self, origin: ConeIdentity) -> Option<SemanticOriginFingerprint> {
        match self.origins.get(&origin) {
            Some(SemanticInputOrigin::Artifact(fingerprint)) => Some(*fingerprint),
            Some(SemanticInputOrigin::Source) | None => None,
        }
    }

    /// Imports an ordered closure of validated identity graphs as one atomic
    /// session mutation.
    ///
    /// All imports run against an isolated session snapshot. The receiver is
    /// replaced only after every origin, canonical key, world-id allocation,
    /// and per-layer remap succeeds. Results preserve the caller's canonical
    /// input order.
    pub fn import_batch(
        &mut self,
        imports: &[SemanticIdentityImport<'_>],
    ) -> Result<Vec<ImportedIdentityLayers>, SemanticIdentityImportError> {
        let mut batch_origins = HashSet::new();
        batch_origins.try_reserve(imports.len()).map_err(|_| {
            SemanticIdentityImportError::Allocation {
                requested_slots: imports.len(),
            }
        })?;
        for import in imports {
            if !batch_origins.insert(import.origin) {
                return Err(SemanticIdentityImportError::DuplicateBatchOrigin {
                    origin: import.origin,
                });
            }
        }

        let mut staged = self.try_clone_for_batch()?;
        let mut imported = Vec::new();
        imported.try_reserve_exact(imports.len()).map_err(|_| {
            SemanticIdentityImportError::Allocation {
                requested_slots: imports.len(),
            }
        })?;
        for import in imports {
            imported.push(staged.import(import.origin, import.fingerprint, import.graph)?);
        }
        *self = staged;
        Ok(imported)
    }

    pub fn import(
        &mut self,
        origin: ConeIdentity,
        fingerprint: SemanticOriginFingerprint,
        graph: &ValidatedIdentityGraph,
    ) -> Result<ImportedIdentityLayers, SemanticIdentityImportError> {
        self.import_input(origin, SemanticInputOrigin::Artifact(fingerprint), graph)
    }

    /// A source Cone contributes its already constructed records once per
    /// snapshot. It has no artifact or later-stage fingerprints.
    pub fn import_source(
        &mut self,
        origin: ConeIdentity,
        graph: &ValidatedIdentityGraph,
    ) -> Result<ImportedIdentityLayers, SemanticIdentityImportError> {
        if self.origins.contains_key(&origin) {
            return Err(SemanticIdentityImportError::OriginConflict { origin });
        }
        self.import_input(origin, SemanticInputOrigin::Source, graph)
    }

    fn import_input(
        &mut self,
        origin: ConeIdentity,
        input: SemanticInputOrigin,
        graph: &ValidatedIdentityGraph,
    ) -> Result<ImportedIdentityLayers, SemanticIdentityImportError> {
        if let Some(existing) = self.origins.get(&origin)
            && *existing != input
        {
            return Err(SemanticIdentityImportError::OriginConflict { origin });
        }

        let declared_count = graph.declared_identity_count();
        let mut entries = Vec::new();
        entries.try_reserve_exact(declared_count).map_err(|_| {
            SemanticIdentityImportError::Allocation {
                requested_slots: declared_count,
            }
        })?;
        for (slot, key) in &graph.canonical_keys {
            if let Some(layer) = {
                let node = IdentityNode {
                    kind: slot.kind,
                    bytes: slot.bytes,
                };
                graph
                    .candidates
                    .get(&node)
                    .and_then(|candidate| candidate.layer)
            } {
                entries.push((*slot, layer, key));
            }
        }
        entries.sort_unstable_by_key(|(slot, _, _)| (slot.kind, slot.bytes));

        if entries.len() != declared_count {
            return Err(SemanticIdentityImportError::MissingCanonicalKey);
        }

        let mut new_count = 0_usize;
        for (key_slot, _, key) in &entries {
            let slot = SemanticIdentitySlot::from_key_slot(*key_slot);
            if let Some(existing) = self.entities.get(&slot) {
                if existing.key_slot.key_type != key_slot.key_type
                    || (!Arc::ptr_eq(&existing.key, key) && !existing.key.equals(key.as_ref()))
                {
                    return Err(SemanticIdentityImportError::IdentityConflict {
                        kind: key_slot.kind,
                        id: key_slot.bytes,
                    });
                }
            } else {
                new_count = new_count
                    .checked_add(1)
                    .ok_or(SemanticIdentityImportError::WorldIdExhausted)?;
            }
        }

        let new_count_u64 =
            u64::try_from(new_count).map_err(|_| SemanticIdentityImportError::WorldIdExhausted)?;
        let next_after_import = self
            .next_world_index
            .checked_add(new_count_u64)
            .ok_or(SemanticIdentityImportError::WorldIdExhausted)?;
        if next_after_import > u64::from(u32::MAX) + 1 {
            return Err(SemanticIdentityImportError::WorldIdExhausted);
        }

        self.entities.try_reserve(new_count).map_err(|_| {
            SemanticIdentityImportError::Allocation {
                requested_slots: new_count,
            }
        })?;
        if !self.origins.contains_key(&origin) {
            self.origins
                .try_reserve(1)
                .map_err(|_| SemanticIdentityImportError::Allocation { requested_slots: 1 })?;
        }

        let mut hir = ImportedIdentityMap::<HirIdentityLayer>::new(origin);
        let mut mir = ImportedIdentityMap::<MirIdentityLayer>::new(origin);
        let mut lir = ImportedIdentityMap::<LirIdentityLayer>::new(origin);
        let counts = entries
            .iter()
            .fold([0_usize; 3], |mut counts, (_, layer, _)| {
                counts[layer.index()] += 1;
                counts
            });
        hir.reserve(counts[0])?;
        mir.reserve(counts[1])?;
        lir.reserve(counts[2])?;

        let mut next_world_index = self.next_world_index;
        for (key_slot, layer, key) in entries {
            let slot = SemanticIdentitySlot::from_key_slot(key_slot);
            let world_index = if let Some(existing) = self.entities.get(&slot) {
                existing.world_index
            } else {
                let world_index = u32::try_from(next_world_index)
                    .map_err(|_| SemanticIdentityImportError::WorldIdExhausted)?;
                next_world_index += 1;
                self.entities.insert(
                    slot,
                    SessionEntity {
                        world_index,
                        key_slot,
                        key: Arc::clone(key),
                    },
                );
                world_index
            };
            match layer {
                IdentityLayer::Hir => hir.insert(slot, world_index),
                IdentityLayer::Mir => mir.insert(slot, world_index),
                IdentityLayer::Lir => lir.insert(slot, world_index),
            }
        }

        self.next_world_index = next_world_index;
        self.origins.entry(origin).or_insert(input);
        Ok(ImportedIdentityLayers { hir, mir, lir })
    }

    fn try_clone_for_batch(&self) -> Result<Self, SemanticIdentityImportError> {
        let mut origins = HashMap::new();
        origins.try_reserve(self.origins.len()).map_err(|_| {
            SemanticIdentityImportError::Allocation {
                requested_slots: self.origins.len(),
            }
        })?;
        origins.extend(
            self.origins
                .iter()
                .map(|(origin, fingerprint)| (*origin, *fingerprint)),
        );

        let mut entities = HashMap::new();
        entities.try_reserve(self.entities.len()).map_err(|_| {
            SemanticIdentityImportError::Allocation {
                requested_slots: self.entities.len(),
            }
        })?;
        entities.extend(
            self.entities
                .iter()
                .map(|(slot, entity)| (*slot, entity.clone())),
        );

        Ok(Self {
            origins,
            entities,
            next_world_index: self.next_world_index,
        })
    }
}
