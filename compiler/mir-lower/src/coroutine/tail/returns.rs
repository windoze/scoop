use super::*;

pub(super) fn unchanged(
    body: &mir::Body,
    site: &ResumeSite,
    completed: mir::MirVariantRef,
    addressed: &HashSet<mir::LocalId>,
) -> bool {
    if site
        .destination
        .is_some_and(|local| addressed.contains(&local))
    {
        return false;
    }
    let mut aliases = site.destination.into_iter().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut block_id = site.block;
    let mut start = site.statement + 1;
    loop {
        if !seen.insert(block_id) {
            return false;
        }
        let block = &body.blocks[block_id];
        for statement in &block.statements[start..] {
            match &statement.kind {
                mir::StatementKind::ValDecl { local, init } if result(init, site, &aliases) => {
                    // A newly initialized ordinary local cannot be observed
                    // through storage outside this return path.
                    if body.locals[*local].ty != site.result || addressed.contains(local) {
                        return false;
                    }
                    aliases.insert(*local);
                }
                mir::StatementKind::Expr(value)
                    if matches!(value.kind, mir::ExprKind::UnitLiteral) => {}
                _ => return false,
            }
        }
        match &block.terminator {
            mir::Terminator::Goto(next) => {
                block_id = *next;
                start = 0;
            }
            mir::Terminator::Return { value } => {
                return matches!(&value.kind,
                    mir::ExprKind::VariantConstruct { variant, fields }
                    if *variant == completed && matches!(fields.as_slice(), [value]
                        if result(value, site, &aliases)));
            }
            _ => return false,
        }
    }
}

fn result(expression: &mir::Expr, site: &ResumeSite, aliases: &HashSet<mir::LocalId>) -> bool {
    if expression.ty != site.result {
        return false;
    }
    match expression.kind {
        mir::ExprKind::Local(local) => aliases.contains(&local),
        mir::ExprKind::UnitLiteral => site.destination.is_none() && site.result == mir::Type::Unit,
        _ => false,
    }
}
