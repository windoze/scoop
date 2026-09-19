use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultBindingShapeV1, node, children, {
    let DecodedDefaultBindingShapeV1(field_0) = node;
    children.push(field_0)?;
    Ok(())
});

resource_node!(DecodedDefaultBindingShapeKindV1, node, children, {
    match node {
        Self::Binding(field_0) => {
            children.push(field_0)?;
        }
        Self::Wildcard => return Ok(()),
        Self::Tuple(field_0) => {
            children.push(field_0)?;
        }
        Self::Struct { owner_type, fields } => {
            children.push(fields)?;
            children.push(owner_type)?;
        }
        Self::Class {
            owner_type,
            components,
        } => {
            children.push(components)?;
            children.push(owner_type)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultBindingStructFieldV1, node, children, {
    let DecodedDefaultBindingStructFieldV1 {
        declaration_index,
        shape,
    } = node;
    children.push(shape)?;
    children.push(declaration_index)?;
    Ok(())
});

resource_node!(DecodedDefaultBindingClassComponentV1, node, children, {
    let DecodedDefaultBindingClassComponentV1 { index, shape } = node;
    children.push(shape)?;
    children.push(index)?;
    Ok(())
});
