use std::borrow::Cow;

use scoop_hir::{
    CrossConeHirInterfaceSectionV1, DefaultTemplateDeclarationContractV1,
    DefaultTemplateProviderParameterV1, DefaultTemplateProviderShapeV1, PersistentLexicalRootV1,
    PublicDeclarationOwnerV1,
};
use scoop_identity::{
    CallableTemplateOrigin, StructuralDefinitionPath, StructuralDefinitionSiteRole,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{DefaultNominalShapes, Error};

#[derive(Clone, Copy, Debug)]
pub(super) enum ParameterSelection {
    Position(u32),
    DefaultOrdinal(u32),
}

impl ParameterSelection {
    pub(super) fn from_path(path: &StructuralDefinitionPath) -> Result<Self, Error> {
        match path.segments() {
            [segment] if segment.site_role() == StructuralDefinitionSiteRole::DefaultValue => {
                Ok(Self::DefaultOrdinal(segment.ordinal()))
            }
            _ => Err(Error::DefinitionPath),
        }
    }
}

pub(super) fn contract<'a>(
    interface: &'a CrossConeHirInterfaceSectionV1,
    declaration: CallableTemplateOrigin,
    selection: ParameterSelection,
    shapes: &DefaultNominalShapes<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<DefaultTemplateDeclarationContractV1<'a>, Error> {
    meter.charge_work(
        u64::from(
            interface
                .callable_interfaces()
                .declaration_count()
                .max(1)
                .ilog2(),
        ) + 1,
        path,
    )?;
    let callable = interface
        .callable_interfaces()
        .declaration(declaration)
        .ok_or(Error::MissingDeclaration(declaration))?;
    meter.charge_work(
        u64::from(interface.source_interfaces().records().len().max(1).ilog2()) + 1,
        path,
    )?;
    let protocol = interface
        .source_interfaces()
        .get(declaration)
        .ok_or(Error::MissingProtocol(declaration))?;
    let parameters = protocol.parameters().parameters();
    meter.check_table_entries(parameters.len() as u64, path)?;
    meter.charge_work(parameters.len() as u64 * 2 + 1, path)?;
    let position = match selection {
        ParameterSelection::Position(position) => position as usize,
        ParameterSelection::DefaultOrdinal(ordinal) => parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.calling().template().is_some())
            .nth(ordinal as usize)
            .map(|(position, _)| position)
            .ok_or(Error::DefinitionPath)?,
    };
    if !parameters
        .get(position)
        .is_some_and(|parameter| parameter.calling().template().is_some())
    {
        return Err(Error::MissingDefaultParameter {
            declaration,
            position,
        });
    }
    let ordinal = parameters[..position]
        .iter()
        .filter(|parameter| parameter.calling().template().is_some())
        .count() as u32;
    let outer_arity = match callable.owner() {
        PublicDeclarationOwnerV1::Nominal(owner) => shapes.get(owner)?.type_parameter_arity(),
        PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension => 0,
    };
    let shape =
        DefaultTemplateProviderShapeV1::try_new(outer_arity, callable.type_parameters().len_u32())
            .map_err(Error::Shape)?;
    let root = PersistentLexicalRootV1::try_from(declaration)
        .map_err(|_| Error::MissingDeclaration(declaration))?;
    let receiver = match callable.owner() {
        PublicDeclarationOwnerV1::Nominal(owner) => shape
            .nominal_source_receiver(root, owner, meter, path)
            .map_err(Error::ReceiverShape)?
            .map(Cow::Owned),
        PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension => {
            callable.receiver().map(Cow::Borrowed)
        }
    };
    Ok(DefaultTemplateDeclarationContractV1::new(
        declaration,
        shape,
        DefaultTemplateProviderParameterV1::try_new(callable.parameters(), position as u32)
            .map_err(Error::Parameter)?,
        ordinal,
        receiver,
        callable.effects().execution(),
    ))
}
