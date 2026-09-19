use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultCallableDeclarationV1, node, children, {
    match node {
        Self::Function(field_0) => {
            children.push(field_0)?;
        }
        Self::GenericFunction(field_0) => {
            children.push(field_0)?;
        }
        Self::PropertyAccessor(field_0) => {
            children.push(field_0)?;
        }
        Self::Generated(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultCallableRefV1, node, children, {
    let DecodedDefaultCallableRefV1 {
        declaration,
        owner,
        type_arguments,
    } = node;
    children.push(type_arguments)?;
    children.push(owner)?;
    children.push(declaration)?;
    Ok(())
});

resource_node!(DecodedDefaultBoundCallableSourceV1, node, children, {
    match node {
        Self::Class { bound, callable } => {
            children.push(callable)?;
            children.push(bound)?;
        }
        Self::Interface { bound, member } => {
            children.push(member)?;
            children.push(bound)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultBoundCallableRefV1, node, children, {
    let DecodedDefaultBoundCallableRefV1 {
        receiver_parameter,
        source,
        instantiated_signature,
    } = node;
    children.push(instantiated_signature)?;
    children.push(source)?;
    children.push(receiver_parameter)?;
    Ok(())
});

resource_node!(DecodedDefaultMethodCalleeV1, node, children, {
    match node {
        Self::Callable(field_0) => {
            children.push(field_0)?;
        }
        Self::Bound(field_0) => {
            children.push(field_0)?;
        }
        Self::DerivedEquality { owner_type } => {
            children.push(owner_type)?;
        }
    }
    Ok(())
});
