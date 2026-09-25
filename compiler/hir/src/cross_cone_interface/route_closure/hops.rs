use super::*;
use crate::ReexportRouteHopV1;
use scoop_identity::BindableEntity;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_hop<A>(
    binding: PersistentExportBindingId,
    binding_key: &ExportBindingKey,
    route_index: usize,
    hop_index: usize,
    route_hops: &[ReexportRouteHopV1],
    hop: ReexportRouteHopV1,
    authority: &A,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), PublicExportBindingClosureValidationError>
where
    A: PublicExportBindingClosureAuthority,
{
    let hop_key = authority.binding_key(hop.binding(), meter, path)?.ok_or(
        PublicExportBindingClosureValidationError::MissingHopBindingKey {
            binding,
            route: route_index,
            hop: hop_index,
            hop_binding: hop.binding(),
        },
    )?;
    if hop_key.exporter() != hop.exporter() {
        return Err(
            PublicExportBindingClosureValidationError::HopBindingExporterMismatch {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
                expected: hop.exporter(),
                actual: Box::new(hop_key.exporter()),
            },
        );
    }
    if hop_key.target() != binding_key.target() {
        return Err(
            PublicExportBindingClosureValidationError::HopTargetMismatch {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
                expected: binding_key.target(),
                actual: Box::new(hop_key.target()),
            },
        );
    }
    if hop_key.namespace() != binding_key.namespace() {
        return Err(
            PublicExportBindingClosureValidationError::HopNamespaceMismatch {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
                expected: binding_key.namespace(),
                actual: hop_key.namespace(),
            },
        );
    }
    if hop_key.role() != binding_key.role() {
        return Err(PublicExportBindingClosureValidationError::HopRoleMismatch {
            binding,
            route: route_index,
            hop: hop_index,
            hop_binding: hop.binding(),
            expected: binding_key.role(),
            actual: hop_key.role(),
        });
    }

    let surface = authority
        .public_bindings(hop.exporter(), meter, path)?
        .ok_or(
            PublicExportBindingClosureValidationError::MissingProviderSurface {
                binding,
                route: route_index,
                hop: hop_index,
                provider: hop.exporter(),
            },
        )?;
    let record = surface.get_metered(hop.binding(), meter, path)?.ok_or(
        PublicExportBindingClosureValidationError::MissingProviderBinding {
            binding,
            route: route_index,
            hop: hop_index,
            provider: hop.exporter(),
            hop_binding: hop.binding(),
        },
    )?;

    let terminal = hop_index + 1 == route_hops.len();
    match (terminal, record.source()) {
        (true, ExportBindingSourceV1::DeclaredCurrent { declaration }) => {
            require_declared_target(hop.exporter(), hop.binding(), hop_key, *declaration)
        }
        (true, ExportBindingSourceV1::Reexport { .. }) => Err(
            PublicExportBindingClosureValidationError::TerminalIsReexport {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
            },
        ),
        (false, ExportBindingSourceV1::DeclaredCurrent { .. }) => Err(
            PublicExportBindingClosureValidationError::IntermediateIsDeclared {
                binding,
                route: route_index,
                hop: hop_index,
                hop_binding: hop.binding(),
            },
        ),
        (false, ExportBindingSourceV1::Reexport { routes }) => {
            let suffix = &route_hops[hop_index + 1..];
            if routes.contains_exact_suffix_metered(suffix, meter, path)? {
                Ok(())
            } else {
                Err(
                    PublicExportBindingClosureValidationError::MissingRouteSuffix {
                        binding,
                        route: route_index,
                        hop: hop_index,
                        hop_binding: hop.binding(),
                    },
                )
            }
        }
    }
}

pub(super) fn require_declared_target(
    exporter: ConeIdentity,
    binding: PersistentExportBindingId,
    key: &ExportBindingKey,
    declaration: BindableEntity,
) -> Result<(), PublicExportBindingClosureValidationError> {
    if declaration == key.target() {
        Ok(())
    } else {
        Err(
            PublicExportBindingClosureValidationError::DeclaredTargetMismatch {
                exporter,
                binding,
                expected: key.target(),
                actual: Box::new(declaration),
            },
        )
    }
}
