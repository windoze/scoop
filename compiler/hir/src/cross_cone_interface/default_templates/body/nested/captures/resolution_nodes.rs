use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultCaptureV1, node, children, {
    let DecodedDefaultCaptureV1 {
        source_index,
        value_type,
        first_use_origin,
    } = node;
    children.push(first_use_origin)?;
    children.push(value_type)?;
    children.local_index(*source_index)?;
    children.push(source_index)?;
    Ok(())
});

resource_node!(DecodedDefaultCallableBodyTypeArgumentsV1, node, children, {
    let DecodedDefaultCallableBodyTypeArgumentsV1(field_0) = node;
    children.push(field_0)?;
    Ok(())
});

resource_node!(
    DecodedDefaultCallableBodyTypeArgumentsKindV1,
    node,
    children,
    {
        match node {
            Self::Lexical => return Ok(()),
            Self::Explicit(field_0) => {
                children.push(field_0)?;
            }
        }
        Ok(())
    }
);
