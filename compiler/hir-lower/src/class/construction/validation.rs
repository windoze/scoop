//! Constructor signature uniqueness and delegation-cycle diagnostics.

use super::*;
use std::collections::{HashMap, HashSet};

impl Lowerer {
    pub(super) fn check_duplicate_constructor_signatures(
        &mut self,
        classes: &[(ClassId, &ast::ClassDecl, usize)],
        structs: &[(hir::StructId, &ast::StructDecl, usize)],
    ) {
        for &(class, _, file) in classes {
            self.current_file = file;
            let constructors = self.classes[class].constructors.clone();
            for (index, &constructor) in constructors.iter().enumerate() {
                for &previous in &constructors[..index] {
                    let left = self.class_constructors[constructor].parameters.clone();
                    let right = self.class_constructors[previous].parameters.clone();
                    if self.same_constructor_parameter_types(&left, &right) {
                        let signature = self.class_constructor_signature(constructor);
                        self.error(
                            self.class_constructors[constructor].span,
                            format!(
                                "duplicate constructor signature `{signature}` in class `{}`",
                                self.classes[class].name
                            ),
                        );
                    }
                }
            }
        }
        for &(structure, _, file) in structs {
            self.current_file = file;
            let constructors = self.structs[structure].constructors.clone();
            for (index, &constructor) in constructors.iter().enumerate() {
                for &previous in &constructors[..index] {
                    let left = self.struct_constructors[constructor].parameters.clone();
                    let right = self.struct_constructors[previous].parameters.clone();
                    if self.same_constructor_parameter_types(&left, &right) {
                        let signature = self.struct_constructor_signature(constructor);
                        self.error(
                            self.struct_constructors[constructor].span,
                            format!(
                                "duplicate constructor signature `{signature}` in struct `{}`",
                                self.structs[structure].name
                            ),
                        );
                    }
                }
            }
        }
    }

    fn same_constructor_parameter_types(
        &mut self,
        left: &[hir::ConstructorParameter],
        right: &[hir::ConstructorParameter],
    ) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| self.types_equal(left.ty, right.ty))
    }

    pub(super) fn check_class_constructor_cycles(&mut self, owner: ClassId) {
        let mut edges = HashMap::new();
        for &constructor in &self.classes[owner].constructors {
            if let hir::ClassConstructorKind::Secondary {
                delegation: hir::ClassSecondaryDelegation::This { target, .. },
                ..
            } = self.class_constructors[constructor].kind
            {
                edges.insert(
                    constructor,
                    self.class_constructor_applications[target].constructor,
                );
            }
        }
        self.report_class_cycles(&edges);
    }

    fn report_class_cycles(
        &mut self,
        edges: &HashMap<hir::ClassConstructorId, hir::ClassConstructorId>,
    ) {
        let mut reported = HashSet::new();
        let mut starts = edges.keys().copied().collect::<Vec<_>>();
        starts.sort_by_key(|constructor| constructor.into_raw().into_u32());
        for start in starts {
            let mut positions = HashMap::new();
            let mut path = Vec::new();
            let mut current = start;
            while let Some(&next) = edges.get(&current) {
                if let Some(&position) = positions.get(&current) {
                    let cycle = &path[position..];
                    if cycle
                        .iter()
                        .all(|constructor| !reported.contains(constructor))
                    {
                        reported.extend(cycle.iter().copied());
                        let mut signatures = cycle
                            .iter()
                            .map(|constructor| self.class_constructor_signature(*constructor))
                            .collect::<Vec<_>>();
                        signatures.push(signatures[0].clone());
                        self.error(
                            self.class_constructors[current].span,
                            format!("constructor delegation cycle: {}", signatures.join(" -> ")),
                        );
                    }
                    break;
                }
                positions.insert(current, path.len());
                path.push(current);
                current = next;
            }
        }
    }

    pub(super) fn check_struct_constructor_cycles(&mut self, owner: hir::StructId) {
        let mut edges = HashMap::new();
        for &constructor in &self.structs[owner].constructors {
            if let hir::StructConstructorKind::Secondary { ref delegation, .. } =
                self.struct_constructors[constructor].kind
            {
                edges.insert(
                    constructor,
                    self.struct_constructor_applications[delegation.target].constructor,
                );
            }
        }
        let mut reported = HashSet::new();
        let mut starts = edges.keys().copied().collect::<Vec<_>>();
        starts.sort_by_key(|constructor| constructor.into_raw().into_u32());
        for start in starts {
            let mut positions = HashMap::new();
            let mut path = Vec::new();
            let mut current = start;
            while let Some(&next) = edges.get(&current) {
                if let Some(&position) = positions.get(&current) {
                    let cycle = &path[position..];
                    if cycle
                        .iter()
                        .all(|constructor| !reported.contains(constructor))
                    {
                        reported.extend(cycle.iter().copied());
                        let mut signatures = cycle
                            .iter()
                            .map(|constructor| self.struct_constructor_signature(*constructor))
                            .collect::<Vec<_>>();
                        signatures.push(signatures[0].clone());
                        self.error(
                            self.struct_constructors[current].span,
                            format!("constructor delegation cycle: {}", signatures.join(" -> ")),
                        );
                    }
                    break;
                }
                positions.insert(current, path.len());
                path.push(current);
                current = next;
            }
        }
    }

    fn class_constructor_signature(&mut self, constructor: hir::ClassConstructorId) -> String {
        let declaration = self.class_constructors[constructor].clone();
        let name = self.classes[declaration.owner].name.clone();
        let parameters = declaration
            .parameters
            .iter()
            .map(|parameter| self.type_name(parameter.ty))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name}({parameters})")
    }

    fn struct_constructor_signature(&mut self, constructor: hir::StructConstructorId) -> String {
        let declaration = self.struct_constructors[constructor].clone();
        let name = self.structs[declaration.owner].name.clone();
        let parameters = declaration
            .parameters
            .iter()
            .map(|parameter| self.type_name(parameter.ty))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name}({parameters})")
    }
}
