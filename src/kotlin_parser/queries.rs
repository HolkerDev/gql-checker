use anyhow::Result;
use tree_sitter::{Query, QueryError};
use tree_sitter_kotlin::language;

pub fn package_query() -> Result<Query, QueryError> {
    let query_string = r#"
    (package_header
        (identifier) @package_name)
        "#;
    Query::new(&language(), query_string)
}
