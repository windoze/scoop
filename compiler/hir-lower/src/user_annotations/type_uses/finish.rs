use super::*;
use std::collections::{BTreeMap, HashSet};

type Conflict = (
    scoop_identity::SourceIdentity,
    scoop_identity::SourceSpan,
    scoop_identity::SourceIdentity,
    scoop_identity::SourceSpan,
);

struct Composition<'a> {
    uses: &'a HashMap<TypeUseKey, PendingTypeUse>,
    direct: HashMap<TypeUseKey, Vec<hir::TypeAnnotation>>,
    complete: HashMap<TypeUseKey, Vec<TypeAnnotationNode>>,
    active: HashSet<TypeUseKey>,
    reported: HashSet<Conflict>,
}

impl Lowerer {
    pub(crate) fn finish_type_annotations(&mut self) {
        let saved_file = self.current_file;
        self.type_annotations.suspended = true;
        let uses = std::mem::take(&mut self.type_annotations.uses).into_owned();
        let mut keys = uses.keys().copied().collect::<Vec<_>>();
        keys.sort();
        let mut composition = Composition {
            uses: &uses,
            direct: HashMap::new(),
            complete: HashMap::new(),
            active: HashSet::new(),
            reported: HashSet::new(),
        };
        for &key in &keys {
            if let Some((mut context, annotations)) = uses[&key].direct.clone() {
                context.swap(self);
                let bound = self.bind_type_annotations(&annotations);
                context.swap(self);
                composition.direct.insert(key, bound);
            }
        }
        for key in keys {
            let nodes = self.compose_type_annotations(key, &mut composition);
            if nodes.is_empty() {
                continue;
            }
            let usage = &uses[&key];
            self.annotation_metadata
                .type_uses
                .push(hir::SourceAnnotatedTypeUse {
                    identity: self.type_use_identity(key),
                    ty: usage.ty,
                    parameters: usage.parameters.clone(),
                    nodes,
                });
        }
        let mut seen = HashSet::new();
        for (target, slot, key) in std::mem::take(&mut self.type_annotations.sites) {
            let identity = self.type_use_identity(key);
            if seen.insert((target, slot)) && self.annotation_metadata.type_use(&identity).is_some()
            {
                self.annotation_metadata
                    .type_sites
                    .push(hir::SourceAnnotationTypeSite {
                        target,
                        slot,
                        type_use: identity,
                    });
            }
        }
        self.current_file = saved_file;
    }

    fn compose_type_annotations(
        &mut self,
        key: TypeUseKey,
        state: &mut Composition<'_>,
    ) -> Vec<TypeAnnotationNode> {
        if let Some(nodes) = state.complete.get(&key) {
            return nodes.clone();
        }
        let Some(usage) = state.uses.get(&key) else {
            return Vec::new();
        };
        if !state.active.insert(key) {
            self.current_file = key.file;
            self.error(key.span(), "recursive type annotation expansion".into());
            return Vec::new();
        }
        let mut nodes = if let Some(alias) = &usage.alias {
            let template = match &alias.annotations {
                AliasAnnotations::Source(key) => self.compose_type_annotations(*key, state),
                AliasAnnotations::Imported(nodes) => nodes.clone(),
            };
            let arguments = alias
                .arguments
                .iter()
                .map(|part| {
                    part.as_ref().map_or_else(Vec::new, |part| {
                        project_nodes(&self.compose_type_annotations(part.key, state), part)
                    })
                })
                .collect::<Vec<_>>();
            if arguments.iter().all(Vec::is_empty) {
                template
            } else {
                self.current_file = key.file;
                self.type_annotation_signature(alias.target, &alias.parameters, key.span())
                    .map_or(template.clone(), |ty| {
                        hir::substitute_type_annotations(&ty, &template, &arguments)
                    })
            }
        } else {
            usage
                .children
                .iter()
                .flat_map(|part| {
                    project_nodes(&self.compose_type_annotations(part.key, state), part)
                })
                .collect()
        };
        if let Some(direct) = state.direct.get(&key)
            && !direct.is_empty()
        {
            nodes.push(TypeAnnotationNode {
                path: Vec::new(),
                annotations: direct.clone(),
            });
        }
        let mut merged: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for node in nodes {
            merged
                .entry(node.path)
                .or_default()
                .extend(node.annotations);
        }
        let nodes = merged
            .into_iter()
            .map(|(path, annotations)| TypeAnnotationNode { path, annotations })
            .collect::<Vec<_>>();
        self.check_type_annotation_merges(key, &nodes, &mut state.reported);
        state.active.remove(&key);
        state.complete.insert(key, nodes.clone());
        nodes
    }

    fn check_type_annotation_merges(
        &mut self,
        key: TypeUseKey,
        nodes: &[TypeAnnotationNode],
        reported: &mut HashSet<Conflict>,
    ) {
        for node in nodes {
            let mut seen = HashMap::<_, &hir::TypeAnnotation>::new();
            for application in &node.annotations {
                if let Some(previous) = seen.insert(application.annotation, application)
                    && self
                        .annotation_rules(application.annotation)
                        .is_some_and(|rules| !rules.repeatable)
                    && reported.insert((
                        previous.source.clone(),
                        previous.span,
                        application.source.clone(),
                        application.span,
                    ))
                {
                    self.current_file = key.file;
                    let span = if application.source == self.intrinsic_sources[key.file].identity
                        && application.span.start_byte() >= u64::from(key.start)
                        && application.span.end_byte() <= u64::from(key.end)
                    {
                        ast::Span::new(
                            application.span.start_byte() as u32,
                            application.span.end_byte() as u32,
                        )
                    } else {
                        key.span()
                    };
                    self.check_annotation_rules(
                        application.annotation,
                        hir::AnnotationTargetKind::Type,
                        true,
                        &self.annotation_name(application.annotation),
                        span,
                    );
                }
            }
        }
    }
}

fn project_nodes(nodes: &[TypeAnnotationNode], part: &TypeUsePart) -> Vec<TypeAnnotationNode> {
    nodes
        .iter()
        .filter_map(|node| {
            let path = node.path.strip_prefix(part.from.as_slice())?;
            let mut result = part.to.clone();
            result.extend(path);
            Some(TypeAnnotationNode {
                path: result,
                annotations: node.annotations.clone(),
            })
        })
        .collect()
}
