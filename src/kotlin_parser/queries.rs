use anyhow::Result;
use tree_sitter::{Query, QueryError};
use tree_sitter_kotlin::language;

pub fn package_query() -> Result<Query, QueryError> {
    let query_string = r#"
    (package_header
        (identifier) @package_name
    )
    (class_declaration
        (type_identifier) @class_name
        (primary_constructor
            (class_parameter) @field_type
        )
    ) 
        "#;
    Query::new(&language(), query_string)
}
