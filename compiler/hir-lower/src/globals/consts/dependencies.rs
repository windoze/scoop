use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;
use crate::globals::PendingConst;

impl Lowerer {
    pub(super) fn diagnose_const_dependency_cycles(
        &mut self,
        declarations: &[PendingConst<'_>],
    ) -> std::collections::HashSet<usize> {
        let previous_file = self.current_file;
        let previous_owner = self.current_owner;
        let mut graph = Vec::with_capacity(declarations.len());
        for declaration in declarations {
            self.current_file = declaration.file;
            self.current_owner = match declaration.owner {
                hir::PropertyOwner::TopLevel => None,
                hir::PropertyOwner::Object(object) => Some(crate::Owner::Object(object)),
                _ => unreachable!("the const worklist contains only top-level and object values"),
            };
            let ast::PropertyBodySyntax::Const(expression) = &declaration.declaration.body else {
                unreachable!("the const worklist contains only const properties")
            };
            let mut dependencies = Vec::new();
            self.collect_const_dependencies(
                expression,
                declaration.owner,
                declaration.file,
                declarations,
                &mut dependencies,
            );
            graph.push(dependencies);
        }
        self.current_file = previous_file;
        self.current_owner = previous_owner;

        let labels = declarations
            .iter()
            .map(|declaration| self.const_definition_name(declaration))
            .collect::<Vec<_>>();
        let mut states = vec![DependencyState::Pending; declarations.len()];
        let mut stack = Vec::new();
        let mut cycles = Vec::new();
        for index in 0..declarations.len() {
            find_const_cycles(index, &graph, &labels, &mut states, &mut stack, &mut cycles);
        }
        let mut cyclic = std::collections::HashSet::new();
        for (source, span, path, members) in cycles {
            self.current_file = source;
            self.error(
                span,
                format!("const dependency cycle: {}", path.join(" -> ")),
            );
            cyclic.extend(members);
        }
        cyclic
    }

    fn collect_const_dependencies(
        &self,
        expression: &ast::Expr,
        owner: hir::PropertyOwner,
        file: usize,
        declarations: &[PendingConst<'_>],
        dependencies: &mut Vec<(usize, ast::Span, usize)>,
    ) {
        match expression {
            ast::Expr::Var(name) => {
                if let Some(index) =
                    self.find_const_definition(declarations, owner, &name.text, file, true)
                {
                    dependencies.push((index, name.span, file));
                }
            }
            ast::Expr::FieldAccess(access)
                if access.navigation == ast::Navigation::Direct
                    && matches!(access.selector, ast::FieldSelector::Name(_)) =>
            {
                let ast::FieldSelector::Name(name) = &access.selector else {
                    unreachable!("the match guard selected a named field")
                };
                if let Some(target) = self.nominal_qualifier_target(&access.receiver)
                    && let Some(index) =
                        self.find_qualified_const_definition(declarations, target, &name.text, file)
                {
                    dependencies.push((index, name.span, file));
                }
            }
            ast::Expr::Unary { operand, .. } => {
                self.collect_const_dependencies(operand, owner, file, declarations, dependencies)
            }
            ast::Expr::Binary { lhs, rhs, .. } => {
                self.collect_const_dependencies(lhs, owner, file, declarations, dependencies);
                self.collect_const_dependencies(rhs, owner, file, declarations, dependencies);
            }
            ast::Expr::InfixCall { lhs, rhs, .. } => {
                self.collect_const_dependencies(lhs, owner, file, declarations, dependencies);
                self.collect_const_dependencies(rhs, owner, file, declarations, dependencies);
            }
            ast::Expr::MethodCall { receiver, args, .. } => {
                self.collect_const_dependencies(receiver, owner, file, declarations, dependencies);
                for argument in args {
                    self.collect_const_dependencies(
                        &argument.expression,
                        owner,
                        file,
                        declarations,
                        dependencies,
                    );
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DependencyState {
    Pending,
    Visiting,
    Complete,
}

fn find_const_cycles(
    index: usize,
    graph: &[Vec<(usize, ast::Span, usize)>],
    labels: &[String],
    states: &mut [DependencyState],
    stack: &mut Vec<usize>,
    cycles: &mut Vec<(usize, ast::Span, Vec<String>, Vec<usize>)>,
) {
    match states[index] {
        DependencyState::Complete | DependencyState::Visiting => return,
        DependencyState::Pending => {}
    }
    states[index] = DependencyState::Visiting;
    stack.push(index);
    for &(dependency, span, file) in &graph[index] {
        if states[dependency] == DependencyState::Visiting {
            let cycle_start = stack
                .iter()
                .position(|candidate| *candidate == dependency)
                .expect("a visiting const is present on the dependency stack");
            let mut path = stack[cycle_start..]
                .iter()
                .map(|candidate| labels[*candidate].clone())
                .collect::<Vec<_>>();
            path.push(labels[dependency].clone());
            cycles.push((file, span, path, stack[cycle_start..].to_vec()));
        } else {
            find_const_cycles(dependency, graph, labels, states, stack, cycles);
        }
    }
    stack.pop();
    states[index] = DependencyState::Complete;
}
