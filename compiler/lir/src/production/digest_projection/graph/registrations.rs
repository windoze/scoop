use super::*;

impl DigestGraphWriter<'_> {
    pub(in super::super) fn project_registrations(
        mut self,
        registrations: &crate::StrongRegistrationProductionSurfaceV2,
        entry: &EntryProductionSourceV1,
    ) -> Result<DigestFinalizationPlanV1, DigestProjectionError> {
        self.project_safepoints(
            registrations
                .safepoints()
                .registrations()
                .iter()
                .map(|value| (value.site(), value.owner())),
        )?;
        self.project_callables()?;
        self.project_types(registrations.types().registrations().iter().map(|value| {
            (
                value.semantic().exact_type(),
                value.semantic().instance_layout(),
            )
        }))?;
        self.project_immortal_objects(
            registrations
                .immortal_objects()
                .registrations()
                .iter()
                .map(|value| value.object()),
        )?;
        self.project_static_storages(registrations.static_storages().registrations().iter().map(
            |value| {
                (
                    value.semantic().storage(),
                    value.semantic().layout(),
                    value.semantic().scan(),
                )
            },
        ))?;
        self.project_initialization_units(
            registrations
                .initialization_units()
                .registrations()
                .iter()
                .map(|value| (value.semantic().unit(), value.semantic().schedule())),
        )?;
        self.project_entry(entry)?;
        self.project_odr_definitions()?;
        self.project_image()?;
        self.finish()
    }
}
