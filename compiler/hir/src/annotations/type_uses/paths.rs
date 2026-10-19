use super::TypeAnnotationNode;
use crate::AnnotationParameterType;
use scoop_identity::SignatureTypeKey;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeUseStep {
    Argument(u32),
    TupleElement(u32),
    FunctionReceiver,
    FunctionParameter(u32),
    FunctionResult,
    PointerPointee,
    FunctionPointee,
}

/// Copies argument annotations at occurrences of the original binder, before
/// substitution can make distinct parameters share a semantic type.
pub fn substitute_type_annotations(
    template: &SignatureTypeKey,
    nodes: &[TypeAnnotationNode],
    arguments: &[Vec<TypeAnnotationNode>],
) -> Vec<TypeAnnotationNode> {
    let mut result: BTreeMap<_, _> = nodes
        .iter()
        .map(|node| (node.path.clone(), node.annotations.clone()))
        .collect();
    let mut pending = vec![(template, Vec::new())];
    while let Some((ty, path)) = pending.pop() {
        if let SignatureTypeKey::Binder { depth: 0, index } = ty {
            if let Some(nodes) = arguments.get(*index as usize) {
                for node in nodes {
                    let mut nested = path.clone();
                    nested.extend(&node.path);
                    result
                        .entry(nested)
                        .or_default()
                        .extend(node.annotations.clone());
                }
            }
        } else {
            for (step, child) in children(ty) {
                let mut nested = path.clone();
                nested.extend(step);
                pending.push((child, nested));
            }
        }
    }
    result
        .into_iter()
        .map(|(path, annotations)| TypeAnnotationNode { path, annotations })
        .collect()
}

pub fn type_annotation_path_exists(
    ty: &AnnotationParameterType<SignatureTypeKey>,
    path: &[TypeUseStep],
) -> bool {
    let ty = match ty {
        AnnotationParameterType::Value(ty) => ty,
        AnnotationParameterType::Annotation(_) => return path.is_empty(),
        AnnotationParameterType::AnnotationArray { .. } => {
            return path.is_empty() || path == [TypeUseStep::Argument(0)];
        }
    };
    let mut ty = ty;
    let mut path = path;
    while let Some(step) = path.first() {
        if *step == TypeUseStep::FunctionPointee {
            if !matches!(ty, SignatureTypeKey::NativeFunctionPointer { .. }) {
                return false;
            }
            path = &path[1..];
            if path.is_empty() {
                return true;
            }
            let Some((_, child)) = children(ty)
                .into_iter()
                .find(|(steps, _)| steps.get(1) == path.first())
            else {
                return false;
            };
            ty = child;
            path = &path[1..];
        } else {
            let Some((_, child)) = children(ty)
                .into_iter()
                .find(|(steps, _)| steps.as_slice() == [*step])
            else {
                return false;
            };
            ty = child;
            path = &path[1..];
        }
    }
    true
}

fn children(ty: &SignatureTypeKey) -> Vec<(Vec<TypeUseStep>, &SignatureTypeKey)> {
    use TypeUseStep as Step;
    match ty {
        SignatureTypeKey::Nominal(_)
        | SignatureTypeKey::Binder { .. }
        | SignatureTypeKey::Companion(_) => Vec::new(),
        SignatureTypeKey::NominalApplication { arguments, .. } => arguments
            .as_slice()
            .iter()
            .enumerate()
            .map(|(i, ty)| (vec![Step::Argument(i as u32)], ty))
            .collect(),
        SignatureTypeKey::Tuple(elements) => elements
            .as_slice()
            .iter()
            .enumerate()
            .map(|(i, ty)| (vec![Step::TupleElement(i as u32)], ty))
            .collect(),
        SignatureTypeKey::RawPointer(ty) => vec![(vec![Step::PointerPointee], ty)],
        SignatureTypeKey::Function {
            receiver,
            parameters,
            result,
            ..
        } => {
            let mut result_nodes = vec![(vec![Step::FunctionResult], result.as_ref())];
            result_nodes.extend(
                receiver
                    .iter()
                    .map(|ty| (vec![Step::FunctionReceiver], ty.as_ref())),
            );
            result_nodes.extend(
                parameters
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| (vec![Step::FunctionParameter(i as u32)], ty)),
            );
            result_nodes
        }
        SignatureTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            let mut nodes = vec![(
                vec![Step::FunctionPointee, Step::FunctionResult],
                result.as_ref(),
            )];
            nodes.extend(parameters.iter().enumerate().map(|(i, ty)| {
                (
                    vec![Step::FunctionPointee, Step::FunctionParameter(i as u32)],
                    ty,
                )
            }));
            nodes
        }
    }
}
