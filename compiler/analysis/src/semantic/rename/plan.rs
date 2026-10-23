use super::*;

pub(super) struct Plan {
    pub(super) preparation: RenamePreparation,
    positions: Vec<Origin>,
}

struct NameGraph {
    edges: HashMap<Key, HashSet<Key>>,
    declarations: HashMap<Key, Origin>,
}

impl Plan {
    pub(super) fn edits(&self, world: &World, replacement: &str) -> Vec<SourceEdit> {
        self.positions
            .iter()
            .map(|origin| SourceEdit {
                source: world.sources[&origin.source].version,
                span: origin.span,
                replacement: replacement.to_owned(),
            })
            .collect()
    }
}

impl World {
    pub(super) fn plan(&self, origin: &Origin) -> Result<Option<Plan>, RenameError> {
        self.check()?;
        let Some(name) = self.name(origin) else {
            return Ok(None);
        };
        let names = self.named()?;
        let Some(selected) = names.get(origin).filter(|targets| !targets.is_empty()) else {
            return Ok(None);
        };
        if selected
            .iter()
            .any(|key| matches!(key.target, Target::Callable(Callable::Constructor(_))))
        {
            return Ok(None);
        }
        let NameGraph {
            edges: graph,
            declarations,
        } = self.rename_graph(&names)?;
        let mut group = HashSet::new();
        let mut pending = selected.iter().copied().collect::<Vec<_>>();
        while let Some(key) = pending.pop() {
            self.check()?;
            if group.insert(key)
                && let Some(adjacent) = graph.get(&key)
            {
                pending.extend(adjacent);
            }
        }
        let mut has_source = false;
        for key in &group {
            if matches!(key.target, Target::ExportBinding(_)) {
                continue;
            }
            let Some(declaration) = declarations.get(key) else {
                return Err(RenameError::Unavailable(
                    "renamed declaration has no editable project source",
                ));
            };
            if self.name(declaration).as_deref() != Some(&name) {
                return Err(RenameError::Unavailable(
                    "related declarations do not share an editable source name",
                ));
            }
            has_source = true;
        }
        if !has_source {
            return Err(RenameError::Unavailable(
                "renamed declaration has no editable project source",
            ));
        }
        let mut positions = Vec::new();
        for (position, targets) in names {
            self.check()?;
            if targets.is_disjoint(&group) {
                continue;
            }
            if !targets.is_subset(&group) {
                return Err(RenameError::Unavailable(
                    "one source name also refers to a different declaration",
                ));
            }
            if self.name(&position).as_deref() != Some(&name) {
                return Err(RenameError::Unavailable(
                    "symbol has an implicit or operator use without an editable identifier",
                ));
            }
            positions.push(position);
        }
        positions.sort_by(|left, right| {
            (&left.source, left.span.start, left.span.end).cmp(&(
                &right.source,
                right.span.start,
                right.span.end,
            ))
        });
        Ok(Some(Plan {
            preparation: RenamePreparation {
                source: self.sources[&origin.source].version,
                span: origin.span,
                placeholder: name,
            },
            positions,
        }))
    }

    fn rename_graph(
        &self,
        names: &HashMap<Origin, HashSet<Key>>,
    ) -> Result<NameGraph, RenameError> {
        let mut graph = HashMap::<_, HashSet<_>>::new();
        let mut declarations = HashMap::new();
        let mut connect = |left, right| {
            graph.entry(left).or_default().insert(right);
            graph.entry(right).or_default().insert(left);
        };
        for &(left, right) in &self.relations {
            connect(left, right);
        }
        for (cone, index) in &self.cones {
            self.check()?;
            for reference in index
                .occurrences()
                .iter()
                .filter(|reference| reference.is_declaration)
            {
                let origin = self.declaration(&reference.origin)?;
                let key = Key::new(*cone, reference.target);
                if let Some(shared) = names.get(&origin).filter(|shared| shared.contains(&key)) {
                    declarations.insert(key, origin);
                    for &other in shared {
                        connect(key, other)
                    }
                }
            }
            for (alias, declaration) in index.aliases.iter() {
                let key = Key::new(*cone, Target::ImportAlias(alias));
                for &export in &declaration.exports {
                    connect(key, Key::new(*cone, Target::ExportBinding(export)));
                }
            }
            for reference in &index.export_references {
                for route in &reference.routes {
                    let terminal = route
                        .hops()
                        .last()
                        .expect("export routes have a terminal binding")
                        .binding();
                    connect(
                        Key::new(*cone, reference.target),
                        Key::new(*cone, Target::ExportBinding(terminal)),
                    );
                }
            }
            for export in &index.reexports {
                let key = Key::new(*cone, Target::ExportBinding(export.binding));
                for route in &export.routes {
                    let terminal = route
                        .hops()
                        .last()
                        .expect("export routes have a terminal binding")
                        .binding();
                    connect(
                        Key::new(*cone, export.target),
                        Key::new(*cone, Target::ExportBinding(terminal)),
                    );
                    if export.alias.is_none() {
                        connect(
                            key,
                            Key::new(*cone, Target::ExportBinding(route.hops()[0].binding())),
                        );
                    }
                }
            }
        }
        Ok(NameGraph {
            edges: graph,
            declarations,
        })
    }
}
