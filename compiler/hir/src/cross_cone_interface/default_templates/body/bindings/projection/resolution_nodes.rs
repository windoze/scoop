use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultBindingProjectionV1, node, children, {
    match node {
        Self::TupleIndex(field_0) => {
            children.push(field_0)?;
        }
        Self::StructField(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});
