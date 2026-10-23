use super::*;

pub(super) struct World {
    pub(super) cones: Vec<(ConeIdentity, EditingReferenceIndex)>,
    pub(super) sources: HashMap<SourceIdentity, Arc<SourceInput>>,
    pub(super) relations: Vec<(Key, Key)>,
    pub(super) cancellation: Rc<dyn Fn() -> bool>,
}

impl World {
    pub(super) fn load(
        session: &mut SemanticSession,
        snapshot: &AnalysisSnapshot,
        project: &ProjectInput,
        cancellation: Rc<dyn Fn() -> bool>,
    ) -> Result<Self, RenameError> {
        let mut world = Self {
            cones: Vec::new(),
            sources: HashMap::new(),
            relations: Vec::new(),
            cancellation,
        };
        for (&cone, input) in &project.cones {
            let Some(input) = input.source() else {
                continue;
            };
            world.check()?;
            for file in &input.sources {
                let source = snapshot
                    .source(file.source)
                    .map_err(|_| RenameError::Cancelled)?
                    .ok_or(RenameError::Unavailable("rename source is unavailable"))?;
                if source
                    .parse()
                    .map_err(|_| RenameError::Unavailable("rename source cannot be parsed"))?
                    .ast()
                    .is_none()
                {
                    return Err(RenameError::Unavailable(
                        "rename requires complete source syntax",
                    ));
                }
                world.sources.insert(file.identity.clone(), source.clone());
            }
            let analysis = session
                .project_declarations(snapshot, project, cone, world.cancellation.clone())
                .map_err(Clone::clone)?;
            let check = world.cancellation.clone();
            let index = analysis
                .reference_index(move || check())
                .map_err(|_| RenameError::Cancelled)?
                .clone();
            if !index.declarations_available || !index.unavailable.is_empty() {
                return Err(RenameError::Unavailable(
                    "rename requires complete declaration and body references",
                ));
            }
            if analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.severity == scoop_source::DiagnosticSeverity::Error)
            {
                return Err(RenameError::Unavailable(
                    "rename requires successful source analysis",
                ));
            }
            let check = world.cancellation.clone();
            let scoop_hir::EditingValue::Known(members) = analysis
                .member_implementation_index(move || check())
                .map_err(|_| RenameError::Cancelled)?
            else {
                return Err(RenameError::Unavailable(
                    "rename requires complete member relationships",
                ));
            };
            if !members.unavailable.is_empty() {
                return Err(RenameError::Unavailable(
                    "rename requires complete member relationships",
                ));
            }
            for relation in &members.relations {
                let key = |member| {
                    Key::new(
                        cone,
                        match member {
                            scoop_hir::EditingImplementationMember::Callable(callable) => {
                                Target::Callable(Callable::Declaration(callable))
                            }
                            scoop_hir::EditingImplementationMember::Property(property) => {
                                Target::Property(property)
                            }
                        },
                    )
                };
                world
                    .relations
                    .push((key(relation.requirement), key(relation.implementation)));
            }
            world.cones.push((cone, index));
        }
        world.check()?;
        Ok(world)
    }

    pub(super) fn check(&self) -> Result<(), RenameError> {
        if (self.cancellation)() {
            Err(RenameError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub(super) fn declaration(&self, origin: &Origin) -> Result<Origin, RenameError> {
        let mut origin = origin.clone();
        if let Some(source) = self.sources.get(&origin.source) {
            origin.span = source
                .declaration_name_span(origin.span)
                .map_err(|_| RenameError::Unavailable("declaration syntax is unavailable"))?
                .unwrap_or(origin.span);
        }
        Ok(origin)
    }

    pub(super) fn name(&self, origin: &Origin) -> Option<String> {
        let source = self.sources.get(&origin.source)?;
        let spelling = source
            .text
            .as_str()
            .get(origin.span.start as usize..origin.span.end as usize)?;
        scoop_parser::identifier_name(spelling)
    }

    pub(super) fn named(&self) -> Result<HashMap<Origin, HashSet<Key>>, RenameError> {
        let mut names = HashMap::<Origin, HashSet<Key>>::new();
        for (cone, index) in &self.cones {
            self.check()?;
            let mut locations = HashMap::<_, Vec<_>>::new();
            for reference in index.occurrences() {
                locations
                    .entry(&reference.origin)
                    .or_default()
                    .push(reference);
            }
            for (origin, references) in locations {
                let mut targets = index.targets_at(origin).to_vec();
                if targets
                    .iter()
                    .any(|target| matches!(target, Target::Callable(Callable::Constructor(_))))
                {
                    let types = references
                        .iter()
                        .filter_map(|reference| {
                            matches!(reference.target, Target::Type(_)).then_some(reference.target)
                        })
                        .collect::<Vec<_>>();
                    if !types.is_empty() {
                        targets.retain(|target| {
                            !matches!(target, Target::Callable(Callable::Constructor(_)))
                        });
                        targets.extend(types);
                    }
                }
                let origin = if references.iter().any(|reference| reference.is_declaration) {
                    self.declaration(origin)?
                } else {
                    origin.clone()
                };
                names
                    .entry(origin)
                    .or_default()
                    .extend(targets.into_iter().map(|target| Key::new(*cone, target)));
            }
        }
        for targets in names.values_mut() {
            if targets
                .iter()
                .any(|key| matches!(key.target, Target::Type(_)))
            {
                targets.retain(|key| {
                    !matches!(key.target, Target::Callable(Callable::Constructor(_)))
                });
            }
        }
        Ok(names)
    }
}
