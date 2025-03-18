use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use graphql_parser::schema::{Definition, Document, TypeDefinition, parse_schema};
use regex::Regex;
use tree_sitter::{Parser as TreeSitterParser, Query, QueryCursor};
use tree_sitter_kotlin::language;
use walkdir::WalkDir;

mod schema_parser;
use schema_parser::SchemaParser;

#[derive(Parser)]
struct CliParams {
    #[arg(long, default_value = "src/main/resources/graphql")]
    schema_path: PathBuf,
    #[arg(long, default_value = "src/main/kotlin")]
    source_path: PathBuf,
    #[arg(short, long, value_name = "DIR")]
    project_path: PathBuf,
}

enum MismatchType {
    MissingResolver(String), // accepts query name
}

fn main() -> Result<()> {
    println!(
        "{}",
        "🚀 Starting GraphQL resolver validator..."
            .bright_green()
            .bold()
    );

    let cli_params = CliParams::parse();
    let project_dir = std::fs::canonicalize(&cli_params.project_path)
        .context("Failed to resolve project path")?;
    let schema_dir = project_dir.join(&cli_params.schema_path);
    let source_dir = project_dir.join(&cli_params.source_path);

    println!("📁 Schema dir: {}", schema_dir.display().to_string().cyan());
    println!("📂 Source dir: {}", source_dir.display().to_string().cyan());

    println!("{}", "📝 Reading GraphQL schema...".yellow());
    let query_file = schema_dir.join("queries.graphqls");
    let query_content =
        std::fs::read_to_string(query_file).context("❌ Failed to read query file")?;

    println!("{}", "🔍 Parsing schema...".yellow());
    let schema: Document<String> =
        parse_schema(&query_content).context("❌ Failed to parse schema")?;

    println!("{}", "🔎 Extracting query names...".yellow());
    let query_names = get_query_names(&schema);

    println!("{}", "⚙️  Parsing resolvers...".yellow());
    let resolvers = get_resolver_names(&source_dir)?;

    println!("{}", "🔄 Checking for mismatches...".magenta());
    let mut mismatches = Vec::new();

    query_names.iter().for_each(|query_name| {
        if !resolvers.contains(&query_name) {
            mismatches.push(MismatchType::MissingResolver(query_name.clone()));
        }
    });

    if mismatches.is_empty() {
        println!(
            "{}",
            "✅ All queries have proper resolvers!"
                .bright_green()
                .bold()
        );
        println!("{}", "🏁 Validation complete!".bright_green().bold());
        Ok(())
    } else {
        println!("{}", "⚠️  Found missing resolvers:".bright_red().bold());

        mismatches.iter().for_each(|mismatch| match mismatch {
            MismatchType::MissingResolver(query_name) => {
                println!(
                    "   ❌ Query {} doesn't have a proper resolver",
                    query_name.bright_red().underline()
                );
            }
        });

        println!("{}", "🏁 Validation failed!".bright_red().bold());
        Err(anyhow::anyhow!(
            "Found {} mismatches",
            mismatches.len()
        ))
    }
}

const QUERY_NAME: &str = "Query";

// Get all query names from the schema
fn get_query_names(schema: &Document<String>) -> Vec<String> {
    let mut query_names = Vec::new();

    schema
        .definitions
        .iter()
        .filter_map(|def| match def {
            Definition::TypeDefinition(TypeDefinition::Object(obj)) if obj.name == QUERY_NAME => {
                Some(obj)
            }
            _ => None,
        })
        .flat_map(|obj| obj.fields.iter().map(|field| field.name.clone()))
        .for_each(|name| query_names.push(name));

    query_names
}

pub fn get_resolver_names(source_dir: &Path) -> Result<Vec<String>> {
    let mut existing_resolvers: Vec<String> = Vec::new();

    // Initialize tree-sitter parser for Kotlin
    let mut parser = TreeSitterParser::new();
    parser.set_language(&language())?;

    // Simple query to find function declarations
    let query_string = r#"(function_declaration) @function_declaration"#;

    let query = Query::new(&language(), query_string)?;
    let function_idx = query
        .capture_index_for_name("function_declaration")
        .unwrap();

    let schema_mapping_regex = Regex::new(
        r#"@SchemaMapping\s*\(\s*typeName\s*=\s*"([^"]+)"\s*,\s*field\s*=\s*"([^"]+)"\s*\)"#,
    )?;
    let method_name_regex = Regex::new(r#"fun\s+([a-zA-Z0-9_]+)"#)?;

    for entry in WalkDir::new(source_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "kt"))
    {
        let file_path = entry.path();
        let content = fs::read_to_string(file_path).unwrap();

        let tree = parser.parse(&content, None).unwrap();

        // Execute the query
        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(&query, tree.root_node(), content.as_bytes());

        for match_ in matches {
            for capture in match_.captures {
                if capture.index == function_idx {
                    let node = capture.node;
                    let function_text = &content[node.start_byte()..node.end_byte()];

                    // Check if this function has a SchemaMapping annotation
                    if let Some(caps) = schema_mapping_regex.captures(function_text) {
                        let type_name = caps.get(1).map_or("", |m| m.as_str()).to_string();
                        let field_name = caps.get(2).map_or("", |m| m.as_str()).to_string();

                        // We only want to process Query resolvers for now
                        if type_name != "Query" {
                            continue;
                        }

                        if let Some(method_caps) = method_name_regex.captures(function_text) {
                            let _method_name =
                                method_caps.get(1).map_or("", |m| m.as_str()).to_string();

                            if existing_resolvers.contains(&field_name) {
                                continue;
                            }
                            existing_resolvers.push(field_name.clone());
                        }
                    }
                }
            }
        }
    }

    Ok(existing_resolvers)
}
