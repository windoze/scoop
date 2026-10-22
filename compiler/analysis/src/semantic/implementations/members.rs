use std::collections::{HashMap, HashSet};

use super::*;
use scoop_hir::{
    EditingImplementationMember as Member, EditingMemberImplementation, SemanticSourceLocation,
};

#[derive(Debug)]
pub struct MemberImplementationSearch {
    pub implementations: Vec<EditingMemberImplementation>,
    pub unavailable: Vec<SemanticSourceLocation>,
    pub declarations_available: bool,
}

impl SemanticSession {
    pub fn member_implementations(
        &mut self,
        snapshot: &AnalysisSnapshot,
        source: SourceId,
        span: Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<MemberImplementationSearch>, ProjectAnalysisError> {
        snapshot.check_cancelled()?;
        let Some(project) = snapshot.workspace.project_for_source(source) else {
            return Ok(None);
        };
        let (current, cone) = project.source_cone(source).expect("selected source Cone");
        let file = cone
            .sources
            .iter()
            .find(|file| file.source == source)
            .expect("selected source");
        let revision = snapshot.clone();
        let check: Rc<dyn Fn() -> bool> =
            Rc::new(move || is_cancelled() || revision.check_cancelled().is_err());
        if check() {
            return Err(ProjectAnalysisError::Cancelled);
        }
        let targets = {
            let analysis = self
                .project_declarations(snapshot, project, current, check.clone())
                .map_err(Clone::clone)?;
            let cancellation = check.clone();
            match analysis
                .member_implementation_targets(&file.identity, span, move || cancellation())
                .map_err(|_| ProjectAnalysisError::Cancelled)?
            {
                V::Known(targets) if !targets.is_empty() => targets,
                _ => return Ok(None),
            }
        };
        let mut relations = HashMap::<Member, HashSet<Member>>::new();
        let mut implementations = HashMap::new();
        let mut result = MemberImplementationSearch {
            implementations: Vec::new(),
            unavailable: Vec::new(),
            declarations_available: true,
        };
        for (&cone, input) in &project.cones {
            if input.source().is_none() {
                continue;
            }
            if check() {
                return Err(ProjectAnalysisError::Cancelled);
            }
            let analysis = self
                .project_declarations(snapshot, project, cone, check.clone())
                .map_err(Clone::clone)?;
            match analysis
                .member_implementation_index(|| check())
                .map_err(|_| ProjectAnalysisError::Cancelled)?
            {
                V::Known(index) => {
                    result.unavailable.extend_from_slice(&index.unavailable);
                    for relation in &index.relations {
                        if check() {
                            return Err(ProjectAnalysisError::Cancelled);
                        }
                        relations
                            .entry(relation.requirement)
                            .or_default()
                            .insert(relation.implementation);
                    }
                    for implementation in &index.implementations {
                        implementations
                            .entry(implementation.declaration)
                            .or_insert_with(|| implementation.clone());
                    }
                }
                V::Unavailable(_) => result.declarations_available = false,
            }
        }
        let mut pending = targets;
        let mut seen = HashSet::new();
        while let Some(member) = pending.pop() {
            if check() {
                return Err(ProjectAnalysisError::Cancelled);
            }
            if !seen.insert(member) {
                continue;
            }
            if let Some(implementation) = implementations.remove(&member) {
                result.implementations.push(implementation);
            }
            if let Some(implementations) = relations.get(&member) {
                pending.extend(implementations);
            }
        }
        Ok(Some(result))
    }
}
