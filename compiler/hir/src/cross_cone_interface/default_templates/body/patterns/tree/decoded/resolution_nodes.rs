use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultPatternV1, node, children, {
    let DecodedDefaultPatternV1(field_0) = node;
    children.push(field_0)?;
    Ok(())
});

resource_node!(DecodedDefaultPatternKindV1, node, children, {
    match node {
        Self::Binding { local_index } => {
            children.local_index(*local_index)?;
            children.push(local_index)?;
        }
        Self::Wildcard => return Ok(()),
        Self::Literal {
            value,
            equality,
            subject_type,
        } => {
            children.push(subject_type)?;
            children.push(equality)?;
            children.push(value)?;
        }
        Self::Variant { variant, fields } => {
            children.push(fields)?;
            children.push(variant)?;
        }
        Self::Tuple { elements } => {
            children.push(elements)?;
        }
        Self::Struct { owner_type, fields } => {
            children.push(fields)?;
            children.push(owner_type)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultPatternFieldV1, node, children, {
    let DecodedDefaultPatternFieldV1 {
        declaration_index,
        pattern,
    } = node;
    children.push(pattern)?;
    children.push(declaration_index)?;
    Ok(())
});
