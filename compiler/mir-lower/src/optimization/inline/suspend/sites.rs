use super::*;

pub(super) fn splice(
    calls: &mut Vec<CallSite>,
    order: usize,
    callee: &[CallSite],
    expansion: &clone::Expansion,
) {
    let original = calls[order].clone();
    for site in calls.iter_mut() {
        if site.block == original.block && site.statement > original.statement {
            site.block = expansion.continuation;
            site.statement = site.statement - original.statement - 1 + expansion.result_prefix;
        }
    }
    // Deferred registrations at the same position precede the selected call.
    // Keep them in the prefix and preserve the source call order of the clones.
    let cloned = callee.iter().map(|site| CallSite {
        block: expansion.blocks[index(site.block)].expect("the complete template was cloned"),
        statement: site.statement,
        pending: original.pending.clone(),
        kind: site.kind.clone(),
    });
    calls.splice(order..=order, cloned);
}
