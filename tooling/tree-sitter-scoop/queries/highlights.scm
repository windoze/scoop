; General names are refined by the syntactic roles below.
(identifier) @variable
(type_identifier) @type

(package_header (qualified_identifier (identifier) @module))
(import_header (qualified_identifier (identifier) @module))
(import_header alias: (identifier) @module)

(function_declaration name: (identifier) @function)
(class_body (function_declaration name: (identifier) @function.method))
(enum_body (function_declaration name: (identifier) @function.method))
(class_declaration name: (identifier) @type.class)
(interface_declaration name: (identifier) @type.interface)
(struct_declaration name: (identifier) @type.struct)
(enum_declaration name: (identifier) @type.enum)
(object_declaration name: (identifier) @type.class)
(companion_object name: (identifier) @type.class)
(annotation_class_declaration name: (identifier) @type.class)
(type_alias name: (identifier) @type)
(type_parameter name: (identifier) @type.parameter)
(enum_variant name: (identifier) @constant)
(parameter name: (identifier) @variable.parameter)
(class_parameter name: (identifier) @variable.member)
(context_parameter name: (identifier) @variable.parameter)
(lambda_parameter name: (identifier) @variable.parameter)
(accessor parameter: (identifier) @variable.parameter)
(argument name: (identifier) @variable.parameter)
(requirement_argument name: (identifier) @variable.parameter)
(enum_field name: (identifier) @variable.member)
(field_pattern name: (identifier) @variable.member)
(field_update name: (identifier) @variable.member)
(navigation_expression member: (identifier) @variable.member)
(property_declaration "val" name: (identifier) @variable.readonly)
(call_expression function: (identifier) @function.call)
(call_expression function: (user_type (type_identifier) @function.call (type_arguments)? .))
(call_expression function: (navigation_expression member: (identifier) @function.method.call))
(callable_reference name: (identifier) @function)
(infix_expression operator: (identifier) @function.call)
(required_call target: (qualified_identifier (identifier) @function.call .))
(annotation name: (qualified_identifier (identifier) @attribute))
(annotation target: (identifier) @attribute)
(label) @label
(this_expression) @variable.builtin
(super_expression) @variable.builtin
(return_expression label: (identifier) @label)
(jump_expression label: (identifier) @label)

((type_identifier) @type.builtin
 (#match? @type.builtin "^(Any|Nothing|Unit|Boolean|Char|String|Int8|Int16|Int32|Int64|Int|Long|Byte|Short|UInt8|UInt16|UInt32|UInt64|UInt|ULong|UByte|UShort|Float|Double|Float32|Float64|Array|MutableArray|Option|Ptr|FunPtr)$"))

["package" "import" "typealias" "class" "struct" "enum" "interface" "object"
 "companion" "annotation" "fun" "val" "var" "constructor" "init" "release"
 "context" "where" "requires" "get" "set" "by" "reified" "vararg"
 "crossinline" "noinline" "suspend" "public" "internal" "private" "protected"
 "override"] @keyword
["if" "else" "when" "case"] @keyword.conditional
["for" "while" "do" "break" "continue"] @keyword.repeat
["try" "catch" "finally" "throw"] @keyword.exception
"return" @keyword.return
["is" "!is" "in" "!in" "as" "as?"] @keyword.operator
(modifier) @keyword.modifier

["=" "+" "-" "*" "/" "%" "==" "!=" "===" "!==" "<" ">" "<=" ">="
 "&&" "||" "!" "!!" "++" "--" "+=" "-=" "*=" "/=" "%=" "?:" "?."
 ".." "..<" "->" "?" "::"] @operator
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["." "," ":" ";" "@"] @punctuation.delimiter
(type_arguments ["<" ">"] @punctuation.bracket)
(type_parameters ["<" ">"] @punctuation.bracket)

(integer_literal) @number
(float_literal) @number.float
(boolean_literal) @boolean
(character_literal) @character
(string_literal) @string
(raw_string_literal) @string
(fstring_content) @string
(raw_fstring_content) @string
(interpolated_string ["f\"" "\""] @string)
(raw_interpolated_string ["f\"\"\"" "\"\"\""] @string)
(escape_sequence) @string.escape
(interpolation ["${" "}"] @punctuation.special)

[(line_comment) (block_comment)] @comment
((line_comment) @comment.documentation
 (#match? @comment.documentation "^///([^/]|$)"))
((block_comment) @comment.documentation
 (#match? @comment.documentation "^/\\*\\*([^*]|$)"))
