//! The same requirement classification order used by object production.

use super::*;

impl SlibObjects {
    pub fn callables(
        &self,
        stackmaps: ScoopStackmapSetV1,
    ) -> VerifiedStrongCallableFingerprintSetV1 {
        let sites = self.sites(stackmaps.builtins().clone());
        let requirements = self.requirements(sites.clone());
        let objects = candidates(&self.objects);
        let registrations = bind_callable_registrations_v1(
            sites,
            self.emitted
                .production()
                .registration_production()
                .callables()
                .clone(),
        )
        .unwrap();
        let bodies = compute_strong_callable_body_object_fingerprints_v1(
            registrations,
            stackmaps,
            requirements,
            &objects,
        )
        .expect("real ELF callable body fingerprints");
        compute_strong_callable_fingerprints_v1(
            bodies,
            self.emitted.production().canonical_callable_definitions(),
        )
        .expect("real ELF callable definition and registration fingerprints")
    }

    pub fn requirements(
        &self,
        sites: ScoopLirDigestPatchSitesV1,
    ) -> CanonicalUndefinedSymbolRequirementSetV1 {
        let strong = sites.builtins().strong_relocations().clone();
        let bridges = self.emitted.production().generated_bridge_plan().clone();
        let current = verify_current_cone_undefined_requirements_v1(strong.clone(), bridges)
            .expect("current Cone requirements");
        let native = scoop_lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            self.emitted.target(),
            self.emitted.foundation(),
        )
        .unwrap();
        let dependencies =
            verify_dependency_strong_requirements_v1(self.emitted.target(), strong, &[])
                .expect("dependency requirements");
        let source = verify_source_external_requirements_v1(dependencies, native.clone())
            .expect("source external requirements");
        let runtime =
            verify_runtime_and_eh_requirements_v1(source, self.emitted.target_selection())
                .expect("runtime and EH requirements");
        finalize_undefined_symbol_requirements_v1(current, runtime)
    }
}
