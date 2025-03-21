use anyhow::Result;
use graphql_parser::{
    parse_schema,
    schema::{Definition, Document, TypeDefinition},
};
use std::{ops::Not, path::PathBuf};
use walkdir::WalkDir;

const QUERY_NAME: &str = "Query";

#[derive(Debug)]
pub struct SchemaParser {
    queries: Vec<Query>,
    custom_scalars: Vec<String>,
}

#[derive(Debug)]
pub struct Query {
    pub name: String,
    pub arguments: Vec<Argument>,
}

#[derive(Clone, Debug)]
pub struct Argument {
    pub name: String,
    pub value_type: String,
    pub is_nullable: bool,
}

impl SchemaParser {
    pub fn new(schema_dir: PathBuf) -> Result<Self> {
        let schema_dir = schema_dir.clone();
        let mut custom_scalars: Vec<String> = Vec::new();
        let mut schema_queries: Vec<Query> = Vec::new();

        for entry in WalkDir::new(schema_dir.clone())
            .into_iter()
            .filter_map(|e| e.ok())
        {
            // skip directories, because I assume all files are not nested
            if entry.path().is_file().not() {
                continue;
            }

            let content = std::fs::read_to_string(entry.path())?;

            let schema = parse_schema::<String>(&content)?;

            let scalars = Self::extract_custom_scalars(&schema);
            custom_scalars.extend(scalars);

            let queries = Self::extract_queries(&schema);
            schema_queries.extend(queries);
        }

        for query in &mut schema_queries {
            query.arguments = query
                .arguments
                .iter()
                .filter(|arg| custom_scalars.contains(&arg.value_type).not())
                .cloned()
                .collect();
        }

        Ok(Self {
            queries: schema_queries,
            custom_scalars,
        })
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
            .collect::<Vec<String>>()
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
                            let is_nullable = !arg.value_type.to_string().contains("!");
                            let value_type = arg.value_type.to_string().replace("!", "");
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
            .collect::<Vec<Query>>()
    }

    pub fn get_query_names(&self) -> Vec<String> {
        self.queries.iter().map(|q| q.name.clone()).collect()
    }

    pub fn get_queries(&self) -> &Vec<Query> {
        &self.queries
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
