use anyhow::{Context, Result};
use graphql_parser::{
    parse_schema,
    schema::{Definition, Document, TypeDefinition},
};
use std::{ops::Not, path::PathBuf};
use walkdir::WalkDir;

/// Custom error types for schema parsing operations
#[derive(Debug, thiserror::Error)]
pub enum SchemaParserError {
    #[error("Failed to read schema file: {0}")]
    FileReadError(#[from] std::io::Error),
    #[error("Failed to parse GraphQL schema: {0}")]
    ParseError(String),
    #[error("No schema files found in directory: {0}")]
    NoSchemaFiles(PathBuf),
    #[error("Invalid schema directory: {0}")]
    InvalidSchemaDir(String),
}

/// The name of the root Query type in GraphQL schema
const QUERY_NAME: &str = "Query";

/// A parser for GraphQL schema files that extracts queries and custom scalars
#[derive(Debug)]
pub struct SchemaParser {
    queries: Vec<Query>,
    custom_scalars: Vec<String>,
}

/// Represents a GraphQL query with its name and arguments
#[derive(Debug)]
pub struct Query {
    /// The name of the query
    pub name: String,
    /// The arguments accepted by the query
    pub arguments: Vec<Argument>,
}

/// Represents a GraphQL argument with its name, type, and nullability
#[derive(Clone, Debug)]
pub struct Argument {
    /// The name of the argument
    pub name: String,
    /// The GraphQL type of the argument
    pub value_type: String,
    /// Whether the argument can be null
    pub is_nullable: bool,
}

impl SchemaParser {
    /// Creates a new SchemaParser by reading and parsing GraphQL schema files from the given directory
    ///
    /// # Arguments
    /// * `schema_dir` - Path to the directory containing GraphQL schema files
    ///
    /// # Returns
    /// * `anyhow::Result<Self>` - The constructed SchemaParser or an error if parsing fails
    pub fn new(schema_dir: PathBuf) -> anyhow::Result<Self, SchemaParserError> {
        if !schema_dir.exists() {
            return Err(SchemaParserError::InvalidSchemaDir(
                "Schema directory does not exist".to_string(),
            )
            .into());
        }

        if !schema_dir.is_dir() {
            return Err(SchemaParserError::InvalidSchemaDir(
                "Schema path is not a directory".to_string(),
            )
            .into());
        }

        let mut custom_scalars: Vec<String> = Vec::new();
        let mut schema_queries: Vec<Query> = Vec::new();
        let mut found_schema_files = false;

        for entry in WalkDir::new(&schema_dir).into_iter().filter_map(|e| e.ok()) {
            if !entry.path().is_file() || !entry.path().to_string_lossy().ends_with(".graphqls") {
                continue;
            }

            found_schema_files = true;
            let content = std::fs::read_to_string(entry.path())
                .map_err(|e| SchemaParserError::FileReadError(e))?;

            let schema = parse_schema::<String>(&content)
                .map_err(|e| SchemaParserError::ParseError(e.to_string()))?;

            custom_scalars.extend(Self::extract_custom_scalars(&schema));
            schema_queries.extend(Self::extract_queries(&schema));
        }

        if !found_schema_files {
            return Err(SchemaParserError::NoSchemaFiles(schema_dir).into());
        }

        // Filter out custom scalar arguments from queries
        for query in &mut schema_queries {
            query.arguments = query
                .arguments
                .iter()
                .filter(|arg| !custom_scalars.contains(&arg.value_type))
                .cloned()
                .collect();
        }

        Ok(Self {
            queries: schema_queries,
            custom_scalars,
        })
    }

    /// Returns a list of all query names found in the schema
    pub fn get_query_names(&self) -> Vec<String> {
        self.queries.iter().map(|q| q.name.clone()).collect()
    }

    /// Returns all queries found in the schema
    pub fn get_queries(&self) -> &[Query] {
        &self.queries
    }

    fn extract_custom_scalars(schema: &Document<String>) -> Vec<String> {
        schema
            .definitions
            .iter()
            .filter_map(|def| {
                if let Definition::TypeDefinition(TypeDefinition::Scalar(scalar)) = def {
                    Some(scalar.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn extract_queries(schema: &Document<String>) -> Vec<Query> {
        schema
            .definitions
            .iter()
            .filter_map(|def| {
                let obj = match def {
                    Definition::TypeDefinition(TypeDefinition::Object(obj)) => obj,
                    _ => return None,
                };

                if obj.name != QUERY_NAME {
                    return None;
                }

                Some(&obj.fields)
            })
            .flat_map(|fields| {
                fields.iter().map(|field| {
                    let arguments = field
                        .arguments
                        .iter()
                        .filter_map(|arg| {
                            let is_nullable = !arg.value_type.to_string().contains('!');
                            let value_type = arg.value_type.to_string().replace('!', "");
                            Some(Argument {
                                name: arg.name.clone(),
                                value_type,
                                is_nullable,
                            })
                        })
                        .collect();

                    Query {
                        name: field.name.clone(),
                        arguments,
                    }
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_query_names() {
        let parser = SchemaParser::new(PathBuf::from("test-files")).unwrap();
        let query_names = parser.get_query_names();
        assert_eq!(query_names, vec!["employee", "searchEmployee"]);
    }

    #[test]
    fn test_returns_correct_queries_ignoring_custom_scalars() {
        let parser = SchemaParser::new(PathBuf::from("test-files")).unwrap();
        let queries = parser.get_queries();
        assert_eq!(queries.len(), 2);

        // employee query
        assert_eq!(queries[0].name, "employee");
        assert_eq!(queries[0].arguments.len(), 1);
        assert_eq!(queries[0].arguments[0].name, "id");
        assert_eq!(queries[0].arguments[0].value_type, "ID");
        assert_eq!(queries[0].arguments[0].is_nullable, false);

        // searchEmployee query
        assert_eq!(queries[1].name, "searchEmployee");
        assert_eq!(queries[1].arguments.len(), 2);
        assert_eq!(queries[1].arguments[0].name, "name");
        assert_eq!(queries[1].arguments[0].value_type, "String");
        assert_eq!(queries[1].arguments[0].is_nullable, false);
        assert_eq!(queries[1].arguments[1].name, "age");
        assert_eq!(queries[1].arguments[1].value_type, "Int");
        assert_eq!(queries[1].arguments[1].is_nullable, true);
    }
}
