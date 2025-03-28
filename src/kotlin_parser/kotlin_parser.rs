use std::{fs, net::ToSocketAddrs, path::Path};

use tree_sitter::{Parser as TreeSitterParser, Query, QueryCursor};
use tree_sitter_kotlin::language;
use walkdir::WalkDir;

use super::queries::package_query;

struct KotlinParser {
    files: Vec<File>,
}

struct File {
    package: String,
    classes: Vec<Class>,
}

struct Class {
    fields: Vec<Field>,
    name: String,
}

struct Field {
    field_type: String,
}

impl KotlinParser {
    pub fn new(source_dir: &Path) -> anyhow::Result<Self> {
        let mut files: Vec<File> = Vec::new();

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
                classes: vec![],
            };

            let file_path = entry.path();
            let content = fs::read_to_string(file_path).unwrap();

            let tree = parser.parse(&content, None).unwrap();
            let query = package_query()?;
            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(&query, tree.root_node(), content.as_bytes());
            for match_ in matches {
                for capture in match_.captures {
                    let capture_name = query.capture_names()[capture.index as usize];
                    let captured_el = &content[capture.node.byte_range()];
                    match capture_name {
                        "package_name" => {
                            file.package = captured_el.to_string();
                        }
                        "class_name" => {
                            file.classes.push(Class {
                                fields: vec![],
                                name: format!("{}.{}", file.package, captured_el.to_string()),
                            });
                        }
                        _ => {
                            println!("Found unhandled capture name: {}", capture_name)
                        }
                    }
                }
            }

            files.push(file);
        }

        Ok(KotlinParser { files })
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

        assert_eq!(parser.files[0].classes.len(), 1);
        assert_eq!(
            parser.files[0].classes[0].name,
            format!("{}.{}", package_name, "TestDataClass")
        );
    }
}
