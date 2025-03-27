use std::{fs, path::Path};

use regex::Regex;
use tree_sitter::{Parser as TreeSitterParser, Query, QueryCursor};
use tree_sitter_kotlin::language;
use walkdir::WalkDir;

struct KotlinParser {
    files: Vec<File>,
}

struct File {
    package: String,
    classes: Vec<Class>,
}

struct Class {
    fields: Vec<Field>,
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

        // Simple query to find function declarations
        let query_string = r#"
            (package_header
                (identifier) @package_name)
        "#;
        let query = Query::new(&language(), query_string)?;

        for entry in WalkDir::new(source_dir).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    println!("Error accessing entry: {}", e);
                    continue;
                }
            };

            if !entry.path().extension().map_or(false, |ext| ext == "kt") {
                println!("Skipping: not a .kt file");
                continue;
            }

            let file_path = entry.path();
            let content = fs::read_to_string(file_path).unwrap();

            let tree = parser.parse(&content, None).unwrap();

            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(&query, tree.root_node(), content.as_bytes());
            for match_ in matches {
                for capture in match_.captures {
                    let captured_text = &content[capture.node.byte_range()];
                    files.push(File {
                        package: captured_text.to_string(),
                        classes: vec![],
                    });
                    println!("Package name: {}", captured_text);
                }
            }
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
        assert_eq!(parser.files.len(), 1);
    }
}
