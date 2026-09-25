use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultExpressionV1, node, children, {
    let DecodedDefaultExpressionV1 {
        kind,
        result_type,
        definition_origin,
    } = node;
    children.push(definition_origin)?;
    children.push(result_type)?;
    children.push(kind)?;
    Ok(())
});

mod expression_kind;

resource_node!(DecodedOptionalDefaultExpressionV1, node, children, {
    match node {
        Self::Absent => return Ok(()),
        Self::Present(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultArrayAssemblyV1, node, children, {
    let DecodedDefaultArrayAssemblyV1 {
        element_type,
        parts,
        result_type,
    } = node;
    children.push(result_type)?;
    children.push(parts)?;
    children.push(element_type)?;
    Ok(())
});

resource_node!(DecodedDefaultArrayAssemblyPartV1, node, children, {
    match node {
        Self::Element(field_0) => {
            children.push(field_0)?;
        }
        Self::CopyArray(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultIntegerArgumentsV1, node, children, {
    match node {
        Self::Unary(field_0) => {
            children.push(field_0)?;
        }
        Self::Binary { lhs, rhs } => {
            children.push(rhs)?;
            children.push(lhs)?;
        }
    }
    Ok(())
});

resource_node!(
    crate::SourceCallReceiver<DecodedSignatureTypeKey>,
    node,
    children,
    {
        match node {
            Self::NoReceiver => Ok(()),
            Self::Receiver { static_type } => children.push(static_type),
        }
    }
);
