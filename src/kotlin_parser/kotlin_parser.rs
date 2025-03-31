use std::{
    collections::{HashMap, HashSet},
    fs,
    net::ToSocketAddrs,
    path::Path,
};

use tree_sitter::{Parser as TreeSitterParser, Query, QueryCursor};
use tree_sitter_kotlin::language;
use walkdir::WalkDir;

use super::queries::package_query;

type ClassName = String;

struct KotlinParser {
    files: Vec<File>,
    class_map: HashMap<ClassName, Class>,
}

struct File {
    package: String,
}

#[derive(Hash, Eq, PartialEq, Debug)]
struct Class {
    fields: Vec<Field>,
    name: String,
}

#[derive(Hash, Eq, PartialEq, Debug)]
struct Field {
    field_type: String,
    field_name: String,
}

impl KotlinParser {
    pub fn new(source_dir: &Path) -> anyhow::Result<Self> {
        let mut files: Vec<File> = Vec::new();
        let mut class_map: HashMap<ClassName, Class> = HashMap::new();

        // Initialize tree-sitter parser for Kotlin
        let mut parser = TreeSitterParser::new();
        parser.set_language(&language())?;

        for entry in WalkDir::new(source_dir).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    println!("Error accessing entry: {}", e);
                    continue;
                }
            };

            if !entry.path().extension().map_or(false, |ext| ext == "kt") {
                continue;
            }

            let mut file = File {
                package: "".to_string(),
            };

            let file_path = entry.path();
            let content = fs::read_to_string(file_path).unwrap();

            let tree = parser.parse(&content, None).unwrap();
            let query = package_query()?;
            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(&query, tree.root_node(), content.as_bytes());

            let mut current_class_name: Option<String> = None;
            for match_ in matches {
                for capture in match_.captures {
                    let capture_name = query.capture_names()[capture.index as usize];
                    let captured_el = &content[capture.node.byte_range()];
                    match capture_name {
                        "package_name" => {
                            file.package = captured_el.to_string();
                        }
                        "class_name" => {
                            let class_name =
                                format!("{}.{}", file.package, captured_el.to_string());
                            // Skip class if it was added
                            if class_map.contains_key(&class_name) {
                                continue;
                            }
                            let found_class = Class {
                                fields: vec![],
                                name: class_name.clone(),
                            };
                            current_class_name = Some(class_name.clone());
                            class_map.insert(class_name, found_class);
                        }
                        "field_type" => {
                            let type_parts: Vec<&str> = captured_el.split(": ").collect();
                            let field_name = type_parts[0]
                                .trim_start_matches("val ")
                                .trim_start_matches("var ")
                                .trim();
                            let field_type = type_parts[1];

                            let class_name = match current_class_name {
                                Some(ref name) => name.clone(),
                                None => continue,
                            };
                            if let Some(class) = class_map.get_mut(&class_name) {
                                class.fields.push(Field {
                                    field_name: field_name.to_string(),
                                    field_type: field_type.to_string(),
                                });
                            }
                        }
                        _ => {
                            println!("Found unhandled capture name: {}", capture_name)
                        }
                    }
                }
            }

            files.push(file);
        }

        Ok(KotlinParser { files, class_map })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_new_kotlin_parser() {
        let test_files_dir = PathBuf::from("test-files");
        let parser = KotlinParser::new(&test_files_dir).unwrap();
        let package_name = "com.example.app";
        assert_eq!(parser.files.len(), 1);
        assert_eq!(parser.files[0].package, package_name);

        assert_eq!(parser.class_map.len(), 1);
        assert_eq!(
            *parser.class_map.iter().next().unwrap().0,
            format!("{}.{}", package_name, "TestDataClass")
        );
        assert_eq!(parser.class_map.iter().next().unwrap().1.fields.len(), 2);
    }
}
