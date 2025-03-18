use graphql_parser::{
    parse_schema,
    schema::{Definition, Document, TypeDefinition},
};
use anyhow::Result;
use std::{ops::Not, path::PathBuf};

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

#[derive(Debug)]
pub struct Argument {
    pub name: String,
    pub value_type: String,
    pub is_nullable: bool,
}

impl SchemaParser {
    pub fn new(schema_dir: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(schema_dir.join("queries.graphqls"))?;
        let schema = parse_schema::<String>(&content)?;
        
        let custom_scalars: Vec<String> = schema.definitions.iter()
            .filter_map(|def| {
                if let Definition::TypeDefinition(TypeDefinition::Scalar(scalar)) = def {
                    Some(scalar.name.clone())
                } else {
                    None
                }
            })
            .collect();

        let queries: Vec<Query> = schema.definitions.iter()
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
                    let arguments = field.arguments.iter()
                        .filter_map(|arg| {
                            let is_nullable = !arg.value_type.to_string().contains("!");
                            let value_type = arg.value_type.to_string().replace("!", "");
                            
                            if custom_scalars.contains(&value_type) {
                                None
                            } else {
                                Some(Argument {
                                    name: arg.name.clone(),
                                    value_type,
                                    is_nullable,
                                })
                            }
                        })
                        .collect();

                    Query {
                        name: field.name.clone(),
                        arguments,
                    }
                })
            })
            .collect();
        
        Ok(Self { queries, custom_scalars })
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
