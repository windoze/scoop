[(block) (class_body) (enum_body) (when_body) (requirements_body)
 (lambda_literal) (parameters) (class_parameters) (arguments)
 (function_type_parameters) (requirement_parameters) (requirement_arguments)
 (type_parameters) (type_arguments) (enum_payload) (enum_fields)
 (field_updates) (field_patterns) (array_literal) (tuple_literal) (tuple_type)
 (interpolation) (delegation_specifiers) (where_clause)] @indent.begin

["}" ")" "]" ">"] @indent.end

; Recovery can leave an opening delimiter directly inside an ERROR node.
(ERROR ["{" "(" "["] @indent.begin)

; The host applies these only to bodies without braces.
(if_expression consequence: (_) @indent.branch)
(if_expression alternative: (_) @indent.branch)
(while_statement body: (_) @indent.branch)
(do_while_statement body: (_) @indent.branch)
(for_statement body: (_) @indent.branch)
(when_entry body: (_) @indent.branch)
(property_declaration (accessor) @indent.branch)
(where_clause) @indent.continuation

(property_declaration value: (_) @indent.continuation)
(property_declaration delegate: (_) @indent.continuation)
(expression_body body: (_) @indent.continuation)
(assignment right: (_) @indent.continuation)
(binary_expression right: (_) @indent.continuation)
(field_update value: (_) @indent.continuation)

; Reindenting raw text would change the value of the string.
[(raw_string_content) (raw_fstring_content) (block_comment)] @indent.ignore
