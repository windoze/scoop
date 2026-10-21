[(block) (class_body) (enum_body) (when_body) (requirements_body)
 (lambda_literal) (parameters) (class_parameters) (arguments)
 (function_type_parameters) (requirement_parameters) (requirement_arguments)
 (type_parameters) (type_arguments) (enum_payload) (enum_fields)
 (field_updates) (field_patterns) (array_literal) (tuple_literal) (tuple_type)
 (raw_string_literal) (raw_interpolated_string)] @fold

(block_comment) @fold.comment
; Hosts can coalesce consecutive imports and standalone line comments.
(import_header) @fold.import
(line_comment) @fold.line_comment
