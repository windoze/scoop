#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SlibClosureLimitProfileIdV1 {
    M23Default,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedSlibClosureLimitsV1 {
    id: SlibClosureLimitProfileIdV1,
    limits: scoop_slib::SlibClosureDecodeLimitsV1,
}

impl ResolvedSlibClosureLimitsV1 {
    pub const M23_DEFAULT: Self = Self {
        id: SlibClosureLimitProfileIdV1::M23Default,
        limits: scoop_slib::SlibClosureDecodeLimitsV1::M23_DEFAULT,
    };

    pub const fn id(self) -> SlibClosureLimitProfileIdV1 {
        self.id
    }

    pub const fn limits(self) -> scoop_slib::SlibClosureDecodeLimitsV1 {
        self.limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_owns_the_single_production_closure_profile() {
        let profile = ResolvedSlibClosureLimitsV1::M23_DEFAULT;
        assert_eq!(profile.id(), SlibClosureLimitProfileIdV1::M23Default);
        assert_eq!(profile.limits().values().cone_nodes, 4_096);
        assert_eq!(profile.limits().values().child_requests, 4_096);
    }
}
