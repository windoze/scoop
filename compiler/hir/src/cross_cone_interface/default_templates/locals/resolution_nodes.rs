use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedTemplateLocalDefinitionV1, node, children, {
    match node {
        Self::Source(field_0) => {
            children.push(field_0)?;
        }
        Self::Synthetic => return Ok(()),
    }
    Ok(())
});

resource_node!(DecodedTemplateLocalRecordV1, node, children, {
    let DecodedTemplateLocalRecordV1 {
        selector,
        value_type,
        mutable,
        definition,
    } = node;
    children.push(definition)?;
    children.push(mutable)?;
    children.push(value_type)?;
    children.push(selector)?;
    Ok(())
});

resource_node!(DecodedCanonicalTemplateLocalTableV1, node, children, {
    let DecodedCanonicalTemplateLocalTableV1 { records } = node;
    children.push(records)?;
    Ok(())
});
