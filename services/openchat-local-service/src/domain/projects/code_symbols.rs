//! Extracts bounded declaration symbols from source files without starting a language server.

use std::path::Path;

use tree_sitter::{Language, Node, Parser};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IndexedSymbol {
    pub(crate) name: String,
    pub(crate) kind: &'static str,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    pub(crate) signature: Option<String>,
    pub(crate) container: Option<String>,
    pub(crate) relationships: Vec<String>,
}

pub(crate) fn extract_symbols(path: &Path, source: &str, limit: usize) -> Vec<IndexedSymbol> {
    let Some((language, family)) = language_for_path(path) else {
        return Vec::new();
    };
    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let mut symbols = Vec::new();
    collect_symbols(
        tree.root_node(),
        family,
        source,
        false,
        None,
        &mut symbols,
        limit,
    );
    symbols
}

#[derive(Clone, Copy)]
enum LanguageFamily {
    Bash,
    C,
    CSharp,
    Cpp,
    Go,
    Java,
    Php,
    Python,
    Rust,
    TypeScript,
}

fn language_for_path(path: &Path) -> Option<(Language, LanguageFamily)> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "bash" | "sh" | "zsh" => (tree_sitter_bash::LANGUAGE.into(), LanguageFamily::Bash),
        "c" | "h" => (tree_sitter_c::LANGUAGE.into(), LanguageFamily::C),
        "cs" => (tree_sitter_c_sharp::LANGUAGE.into(), LanguageFamily::CSharp),
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => {
            (tree_sitter_cpp::LANGUAGE.into(), LanguageFamily::Cpp)
        }
        "go" => (tree_sitter_go::LANGUAGE.into(), LanguageFamily::Go),
        "java" => (tree_sitter_java::LANGUAGE.into(), LanguageFamily::Java),
        "php" => (tree_sitter_php::LANGUAGE_PHP.into(), LanguageFamily::Php),
        "py" | "pyi" => (tree_sitter_python::LANGUAGE.into(), LanguageFamily::Python),
        "rs" => (tree_sitter_rust::LANGUAGE.into(), LanguageFamily::Rust),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => {
            let language = if matches!(extension.as_str(), "tsx" | "jsx") {
                tree_sitter_typescript::LANGUAGE_TSX.into()
            } else {
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
            };
            (language, LanguageFamily::TypeScript)
        }
        _ => return None,
    })
}

pub(crate) fn supports_path(path: &Path) -> bool {
    language_for_path(path).is_some()
}

fn collect_symbols(
    node: Node<'_>,
    family: LanguageFamily,
    source: &str,
    inside_class: bool,
    container: Option<&str>,
    symbols: &mut Vec<IndexedSymbol>,
    limit: usize,
) {
    if symbols.len() >= limit {
        return;
    }
    if let Some(kind) = symbol_kind(family, node.kind(), inside_class)
        && let Some(name_node) = declaration_name(node)
        && let Ok(name) = name_node.utf8_text(source.as_bytes())
    {
        let name = name.trim();
        if !name.is_empty() {
            symbols.push(IndexedSymbol {
                name: name.to_owned(),
                kind,
                start_line: node.start_position().row.saturating_add(1),
                end_line: node.end_position().row.saturating_add(1),
                signature: declaration_signature(node, source, family),
                container: container.map(str::to_owned),
                relationships: declaration_relationships(node, source, family),
            });
        }
    }
    let next_container = if is_container(node.kind()) {
        declaration_name(node)
            .and_then(|name| name.utf8_text(source.as_bytes()).ok())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .or(container)
    } else {
        container
    };
    let next_inside_class = inside_class || is_container(node.kind());
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if symbols.len() >= limit {
            break;
        }
        collect_symbols(
            child,
            family,
            source,
            next_inside_class,
            next_container,
            symbols,
            limit,
        );
    }
}

fn is_container(node_kind: &str) -> bool {
    matches!(
        node_kind,
        "class_definition"
            | "class_declaration"
            | "class_specifier"
            | "struct_declaration"
            | "struct_specifier"
            | "interface_declaration"
            | "record_declaration"
            | "trait_declaration"
    )
}

fn declaration_signature(node: Node<'_>, source: &str, family: LanguageFamily) -> Option<String> {
    let kind = symbol_kind(family, node.kind(), false)?;
    if !matches!(
        kind,
        "function" | "method" | "constructor" | "class" | "interface" | "record"
    ) {
        return None;
    }
    let body_start = ["body", "block", "declaration_list"]
        .iter()
        .filter_map(|field| node.child_by_field_name(field))
        .map(|body| body.start_byte())
        .min()
        .unwrap_or_else(|| node.end_byte());
    if body_start <= node.start_byte() || body_start > source.len() {
        return None;
    }
    let header = source.get(node.start_byte()..body_start)?;
    let signature = compact_text(header, 512);
    (!signature.is_empty()).then_some(signature)
}

fn declaration_relationships(node: Node<'_>, source: &str, family: LanguageFamily) -> Vec<String> {
    if !matches!(
        symbol_kind(family, node.kind(), false),
        Some("class" | "interface" | "record" | "struct" | "trait")
    ) {
        return Vec::new();
    }
    let relationship_nodes: &[&str] = match family {
        LanguageFamily::CSharp => &["base_list"],
        LanguageFamily::Cpp => &["base_class_clause"],
        LanguageFamily::Java => &["superclass", "super_interfaces"],
        LanguageFamily::Php => &["base_clause", "class_interface_clause"],
        LanguageFamily::Python => &["argument_list"],
        LanguageFamily::TypeScript => &["class_heritage"],
        _ => &[],
    };
    let mut relationships = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if !relationship_nodes.contains(&child.kind()) {
            continue;
        }
        if let Ok(value) = child.utf8_text(source.as_bytes()) {
            let value = compact_text(value, 256);
            if !value.is_empty() && !relationships.contains(&value) {
                relationships.push(value);
            }
        }
    }
    relationships
}

fn compact_text(value: &str, max_chars: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

fn symbol_kind(
    family: LanguageFamily,
    node_kind: &str,
    inside_class: bool,
) -> Option<&'static str> {
    Some(match (family, node_kind) {
        (LanguageFamily::Bash, "function_definition") => "function",
        (LanguageFamily::C, "function_definition") => "function",
        (LanguageFamily::C, "struct_specifier") => "struct",
        (LanguageFamily::C, "enum_specifier") => "enum",
        (LanguageFamily::CSharp, "class_declaration") => "class",
        (LanguageFamily::CSharp, "struct_declaration") => "struct",
        (LanguageFamily::CSharp, "interface_declaration") => "interface",
        (LanguageFamily::CSharp, "enum_declaration") => "enum",
        (LanguageFamily::CSharp, "record_declaration") => "record",
        (LanguageFamily::CSharp, "method_declaration") => "method",
        (LanguageFamily::CSharp, "constructor_declaration") => "constructor",
        (LanguageFamily::Cpp, "class_specifier") => "class",
        (LanguageFamily::Cpp, "struct_specifier") => "struct",
        (LanguageFamily::Cpp, "enum_specifier") => "enum",
        (LanguageFamily::Cpp, "namespace_definition") => "namespace",
        (LanguageFamily::Cpp, "function_definition") => "function",
        (LanguageFamily::Go, "function_declaration") => "function",
        (LanguageFamily::Go, "method_declaration") => "method",
        (LanguageFamily::Go, "type_spec") => "type",
        (LanguageFamily::Java, "class_declaration") => "class",
        (LanguageFamily::Java, "interface_declaration") => "interface",
        (LanguageFamily::Java, "enum_declaration") => "enum",
        (LanguageFamily::Java, "record_declaration") => "record",
        (LanguageFamily::Java, "method_declaration") => "method",
        (LanguageFamily::Java, "constructor_declaration") => "constructor",
        (LanguageFamily::Php, "class_declaration") => "class",
        (LanguageFamily::Php, "interface_declaration") => "interface",
        (LanguageFamily::Php, "trait_declaration") => "trait",
        (LanguageFamily::Php, "enum_declaration") => "enum",
        (LanguageFamily::Php, "function_definition") => "function",
        (LanguageFamily::Php, "method_declaration") => "method",
        (LanguageFamily::Python, "class_definition") => "class",
        (LanguageFamily::Python, "function_definition") if inside_class => "method",
        (LanguageFamily::Python, "function_definition") => "function",
        (LanguageFamily::Rust, "function_item") => "function",
        (LanguageFamily::Rust, "struct_item") => "struct",
        (LanguageFamily::Rust, "enum_item") => "enum",
        (LanguageFamily::Rust, "trait_item") => "trait",
        (LanguageFamily::Rust, "type_item") => "type",
        (LanguageFamily::Rust, "mod_item") => "module",
        (LanguageFamily::TypeScript, "class_declaration") => "class",
        (LanguageFamily::TypeScript, "interface_declaration") => "interface",
        (LanguageFamily::TypeScript, "enum_declaration") => "enum",
        (LanguageFamily::TypeScript, "function_declaration") => "function",
        (LanguageFamily::TypeScript, "method_definition") => "method",
        (LanguageFamily::TypeScript, "type_alias_declaration") => "type",
        _ => return None,
    })
}

fn declaration_name(node: Node<'_>) -> Option<Node<'_>> {
    if let Some(name) = node.child_by_field_name("name") {
        return Some(name);
    }
    if matches!(node.kind(), "function_definition" | "function_declaration") {
        return node
            .child_by_field_name("declarator")
            .and_then(first_identifier);
    }
    None
}

fn first_identifier(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(
        node.kind(),
        "identifier" | "type_identifier" | "field_identifier"
    ) {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(identifier) = first_identifier(child) {
            return Some(identifier);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::extract_symbols;

    #[test]
    fn extracts_symbols_from_common_language_families() {
        let samples = [
            (
                Path::new("scripts/build.sh"),
                "function build() { echo done; }\n",
            ),
            (
                Path::new("src/widget.c"),
                "int create_widget() { return 0; }\n",
            ),
            (Path::new("src/widget.cpp"), "class Widget {};\n"),
            (
                Path::new("src/Widget.cs"),
                "class Widget { public void Run() {} }\n",
            ),
            (
                Path::new("src/widget.php"),
                "<?php class Widget {} function run() {}\n",
            ),
            (
                Path::new("src/app.py"),
                "class App:\n    def start(self):\n        pass\n",
            ),
            (
                Path::new("src/app.ts"),
                "export class App { start() {} }\nexport function run() {}",
            ),
            (Path::new("src/app.rs"), "pub struct App;\npub fn run() {}"),
            (
                Path::new("src/app.go"),
                "package app\ntype App struct {}\nfunc Run() {}",
            ),
            (Path::new("src/app.java"), "class App { void run() {} }"),
        ];

        for (path, source) in samples {
            assert!(
                !extract_symbols(path, source, 32).is_empty(),
                "expected symbols from {}",
                path.display()
            );
        }
    }

    #[test]
    fn limits_symbols_and_ignores_unsupported_files() {
        let source = "fn first() {} fn second() {} fn third() {}";
        assert_eq!(extract_symbols(Path::new("src/lib.rs"), source, 2).len(), 2);
        assert!(extract_symbols(Path::new("README.md"), source, 8).is_empty());
    }

    #[test]
    fn extracts_function_signatures_class_relationships_and_containers() {
        let symbols = extract_symbols(
            Path::new("src/app.ts"),
            "class Child extends Base implements Runnable { run(input: string): number { return 1; } }",
            16,
        );
        let class = symbols
            .iter()
            .find(|symbol| symbol.name == "Child")
            .expect("find the class symbol");
        assert_eq!(
            class.signature.as_deref(),
            Some("class Child extends Base implements Runnable")
        );
        assert_eq!(class.relationships, ["extends Base implements Runnable"]);
        let method = symbols
            .iter()
            .find(|symbol| symbol.name == "run")
            .expect("find the method symbol");
        assert_eq!(method.container.as_deref(), Some("Child"));
        assert_eq!(
            method.signature.as_deref(),
            Some("run(input: string): number")
        );
    }

    #[test]
    fn extracts_class_relationships_from_supported_grammars() {
        let samples = [
            (
                Path::new("src/Child.cs"),
                "class Child : Base, IRunnable {}",
            ),
            (Path::new("src/Child.cpp"), "class Child : public Base {};"),
            (
                Path::new("src/Child.java"),
                "class Child extends Base implements Runnable {}",
            ),
            (
                Path::new("src/Child.php"),
                "<?php class Child extends Base implements Runnable {}",
            ),
            (
                Path::new("src/child.py"),
                "class Child(Base, Protocol):\n    pass\n",
            ),
        ];

        for (path, source) in samples {
            let class = extract_symbols(path, source, 16)
                .into_iter()
                .find(|symbol| symbol.name == "Child")
                .unwrap_or_else(|| panic!("expected class symbol from {}", path.display()));
            assert!(
                !class.relationships.is_empty(),
                "expected class relationships from {}",
                path.display()
            );
        }
    }
}
