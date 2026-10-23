use super::*;
use positions::Positions;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Kind {
    Binding,
    Callable,
    Constructor,
    Variant,
    Parameter(u32),
    Property,
    Nominal,
    Alias,
    TypeParameter,
    Annotation,
    ImportAlias,
}

impl Kind {
    fn of(target: Target) -> Option<Self> {
        Some(match target {
            Target::Binding(_) => Self::Binding,
            Target::Callable(Callable::Declaration(_) | Callable::Local(_)) => Self::Callable,
            Target::Callable(Callable::Constructor(_)) => Self::Constructor,
            Target::Callable(Callable::Variant(_)) => Self::Variant,
            Target::Parameter { index, .. } => Self::Parameter(index),
            Target::Property(_) => Self::Property,
            Target::Type(Type::Nominal(_)) => Self::Nominal,
            Target::Type(Type::Alias(_)) => Self::Alias,
            Target::Type(Type::Parameter(_)) => Self::TypeParameter,
            Target::Type(Type::Annotation(_)) => Self::Annotation,
            Target::ImportAlias(_) => Self::ImportAlias,
            Target::ExportBinding(_) => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Anchor {
    Source(Origin, Kind),
    External(Target),
    DirectExport(Box<Anchor>),
    Reexport(Origin, Box<Anchor>),
}

impl Anchor {
    fn mapped(&self, positions: &Positions) -> Result<Self, RenameError> {
        Ok(match self {
            Self::Source(origin, kind) => Self::Source(positions.origin(origin)?, *kind),
            Self::External(target) => Self::External(*target),
            Self::DirectExport(target) => Self::DirectExport(Box::new(target.mapped(positions)?)),
            Self::Reexport(origin, target) => Self::Reexport(
                positions.origin(origin)?,
                Box::new(target.mapped(positions)?),
            ),
        })
    }
}

struct Anchors(HashMap<Key, Anchor>);

impl Anchors {
    fn target(&self, key: Key) -> Result<Anchor, RenameError> {
        if let Some(anchor) = self.0.get(&key) {
            return Ok(anchor.clone());
        }
        if key.arena.is_some() {
            return Err(RenameError::Unavailable(
                "local binding has no source declaration origin",
            ));
        }
        Ok(Anchor::External(key.target))
    }
}

impl World {
    fn anchors(&self) -> Result<Anchors, RenameError> {
        let mut anchors = Anchors(HashMap::new());
        let mut source_keys = HashMap::new();
        for (cone, index) in &self.cones {
            self.check()?;
            for (&target, origin) in &index.definitions {
                let Some(kind) = Kind::of(target) else {
                    continue;
                };
                if !self.sources.contains_key(&origin.source) {
                    continue;
                }
                let key = Key::new(*cone, target);
                let anchor = Anchor::Source(self.declaration(origin)?, kind);
                if source_keys
                    .insert(anchor.clone(), key)
                    .is_some_and(|previous| previous != key)
                {
                    return Err(RenameError::Unavailable(
                        "source declaration origin is ambiguous",
                    ));
                }
                anchors.0.insert(key, anchor);
            }
        }
        for (cone, index) in &self.cones {
            self.check()?;
            for reference in &index.export_references {
                let target = anchors.target(Key::new(*cone, reference.target))?;
                for route in &reference.routes {
                    let binding = route
                        .hops()
                        .last()
                        .expect("export route terminal")
                        .binding();
                    anchors.0.insert(
                        Key::new(*cone, Target::ExportBinding(binding)),
                        Anchor::DirectExport(Box::new(target.clone())),
                    );
                }
            }
            for export in &index.reexports {
                let target = anchors.target(Key::new(*cone, export.target))?;
                for route in &export.routes {
                    let binding = route
                        .hops()
                        .last()
                        .expect("export route terminal")
                        .binding();
                    anchors.0.insert(
                        Key::new(*cone, Target::ExportBinding(binding)),
                        Anchor::DirectExport(Box::new(target.clone())),
                    );
                }
            }
        }
        for (cone, index) in &self.cones {
            for export in &index.reexports {
                let target = anchors.target(Key::new(*cone, export.target))?;
                let origin = export.name.as_ref().unwrap_or(&export.origin).clone();
                anchors.0.insert(
                    Key::new(*cone, Target::ExportBinding(export.binding)),
                    Anchor::Reexport(origin, Box::new(target)),
                );
            }
        }
        Ok(anchors)
    }

    fn bindings(
        &self,
        positions: &Positions,
    ) -> Result<HashMap<Origin, HashSet<Anchor>>, RenameError> {
        let anchors = self.anchors()?;
        let mut bindings = HashMap::<_, HashSet<_>>::new();
        for (cone, index) in &self.cones {
            for reference in index.occurrences() {
                self.check()?;
                let origin = if reference.is_declaration {
                    self.declaration(&reference.origin)?
                } else {
                    reference.origin.clone()
                };
                let target = anchors
                    .target(Key::new(*cone, reference.target))?
                    .mapped(positions)?;
                bindings
                    .entry(positions.origin(&origin)?)
                    .or_default()
                    .insert(target);
            }
        }
        Ok(bindings)
    }

    pub(super) fn verify(&self, candidate: &Self, edits: &[SourceEdit]) -> Result<(), RenameError> {
        let expected = self.bindings(&Positions::new(self, edits))?;
        let actual = candidate.bindings(&Positions::default())?;
        for (origin, targets) in &expected {
            self.check()?;
            if actual.get(origin) != Some(targets) {
                return Err(RenameError::Conflict(origin.clone()));
            }
        }
        if let Some(origin) = actual.keys().find(|origin| !expected.contains_key(*origin)) {
            return Err(RenameError::Conflict(origin.clone()));
        }
        Ok(())
    }
}
