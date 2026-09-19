use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultAssignTargetV1, node, children, {
    match node {
        Self::Local { local_index } => {
            children.local_index(*local_index)?;
            children.push(local_index)?;
        }
        Self::Global { property } => {
            children.push(property)?;
        }
        Self::Index { array, index } => {
            children.push(index)?;
            children.push(array)?;
        }
        Self::Field { receiver, field } => {
            children.push(field)?;
            children.push(receiver)?;
        }
    }
    Ok(())
});
