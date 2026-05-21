//! Migration bridge facts published while typed contracts move into HIR facts.

/// Counts for typed contracts that are still physically owned by the monolithic
/// HIR stage during the P2 migration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TypedContractBridgeFacts {
    pub function_effects: usize,
    pub call_site_contracts: usize,
    pub continuation_resume_sites: usize,
    pub perform_sites: usize,
    pub handle_sites: usize,
    pub assign_place_contracts: usize,
    pub with_update_contracts: usize,
    pub top_level_init_roots: usize,
    pub extern_global_contracts: usize,
}

impl TypedContractBridgeFacts {
    /// Return whether no migration bridge contracts have been published.
    pub fn is_empty(self) -> bool {
        self.function_effects == 0
            && self.call_site_contracts == 0
            && self.continuation_resume_sites == 0
            && self.perform_sites == 0
            && self.handle_sites == 0
            && self.assign_place_contracts == 0
            && self.with_update_contracts == 0
            && self.top_level_init_roots == 0
            && self.extern_global_contracts == 0
    }
}
