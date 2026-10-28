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
    Dart,
    Go,
    Java,
    Lua,
    Php,
    Python,
    Ruby,
    Rust,
    Scala,
    Sql,
    Swift,
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
        "dart" => (tree_sitter_dart::LANGUAGE.into(), LanguageFamily::Dart),
        "go" => (tree_sitter_go::LANGUAGE.into(), LanguageFamily::Go),
        "java" => (tree_sitter_java::LANGUAGE.into(), LanguageFamily::Java),
        "lua" => (tree_sitter_lua::LANGUAGE.into(), LanguageFamily::Lua),
        "php" => (tree_sitter_php::LANGUAGE_PHP.into(), LanguageFamily::Php),
        "py" | "pyi" => (tree_sitter_python::LANGUAGE.into(), LanguageFamily::Python),
        "rb" | "rake" | "gemspec" => (tree_sitter_ruby::LANGUAGE.into(), LanguageFamily::Ruby),
        "rs" => (tree_sitter_rust::LANGUAGE.into(), LanguageFamily::Rust),
        "scala" | "sc" => (tree_sitter_scala::LANGUAGE.into(), LanguageFamily::Scala),
        "sql" => (tree_sitter_sequel::LANGUAGE.into(), LanguageFamily::Sql),
        "swift" => (tree_sitter_swift::LANGUAGE.into(), LanguageFamily::Swift),
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
    if let Some(kind) = declaration_symbol_kind(node, family, inside_class)
        && let Some(name_node) = declaration_name(node, family)
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
    let next_container = if is_container(node.kind())
        || matches!(
            (family, node.kind()),
            (LanguageFamily::TypeScript, "function_declaration")
        )
        || typescript_function_value(node, family).is_some()
    {
        declaration_name(node, family)
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
            | "class"
            | "class_declaration"
            | "class_specifier"
            | "enum_definition"
            | "extension_declaration"
            | "extension_type_declaration"
            | "mixin_declaration"
            | "module"
            | "object_declaration"
            | "object_definition"
            | "protocol_declaration"
            | "singleton_class"
            | "struct_declaration"
            | "struct_specifier"
            | "trait_definition"
            | "interface_declaration"
            | "record_declaration"
            | "trait_declaration"
    )
}

fn declaration_signature(node: Node<'_>, source: &str, family: LanguageFamily) -> Option<String> {
    let kind = declaration_symbol_kind(node, family, false)?;
    if !matches!(
        kind,
        "function" | "method" | "constructor" | "class" | "interface" | "record"
    ) {
        return None;
    }
    let signature_node = typescript_function_value(node, family).unwrap_or(node);
    let body_start = [
        "body",
        "block",
        "declaration_list",
        "function_body",
        "class_body",
        "statements",
    ]
    .iter()
    .filter_map(|field| signature_node.child_by_field_name(field))
    .map(|body| body.start_byte())
    .min()
    .unwrap_or_else(|| signature_node.end_byte());
    if body_start <= node.start_byte() || body_start > source.len() {
        return None;
    }
    let header = source.get(node.start_byte()..body_start)?;
    let signature = compact_text(header, 512);
    (!signature.is_empty()).then_some(signature)
}

fn declaration_relationships(node: Node<'_>, source: &str, family: LanguageFamily) -> Vec<String> {
    if !matches!(
        declaration_symbol_kind(node, family, false),
        Some("class" | "interface" | "record" | "struct" | "trait")
    ) {
        return Vec::new();
    }
    let relationship_nodes: &[&str] = match family {
        LanguageFamily::CSharp => &["base_list"],
        LanguageFamily::Cpp => &["base_class_clause"],
        LanguageFamily::Dart => &["superclass", "interfaces"],
        LanguageFamily::Java => &["superclass", "super_interfaces"],
        LanguageFamily::Php => &["base_clause", "class_interface_clause"],
        LanguageFamily::Python => &["argument_list"],
        LanguageFamily::Scala => &["extend", "derive"],
        LanguageFamily::Swift => &["inheritance_specifier"],
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
        (LanguageFamily::Dart, "class_declaration") => "class",
        (LanguageFamily::Dart, "enum_declaration") => "enum",
        (LanguageFamily::Dart, "extension_declaration") => "extension",
        (LanguageFamily::Dart, "extension_type_declaration") => "type",
        (LanguageFamily::Dart, "function_declaration") => "function",
        (LanguageFamily::Dart, "getter_declaration" | "setter_declaration") => "property",
        (LanguageFamily::Dart, "method_declaration") => "method",
        (LanguageFamily::Dart, "mixin_declaration") => "mixin",
        (LanguageFamily::Dart, "type_alias") => "type",
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
        (LanguageFamily::Lua, "function_declaration") => "function",
        (LanguageFamily::Ruby, "class") => "class",
        (LanguageFamily::Ruby, "method" | "singleton_method") => "method",
        (LanguageFamily::Ruby, "module") => "module",
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
        (LanguageFamily::Scala, "class_definition") => "class",
        (LanguageFamily::Scala, "enum_definition") => "enum",
        (LanguageFamily::Scala, "function_declaration") if inside_class => "method",
        (LanguageFamily::Scala, "function_declaration") => "function",
        (LanguageFamily::Scala, "function_definition") if inside_class => "method",
        (LanguageFamily::Scala, "function_definition") => "function",
        (LanguageFamily::Scala, "object_definition") => "object",
        (LanguageFamily::Scala, "trait_definition") => "trait",
        (LanguageFamily::Scala, "type_definition") => "type",
        (LanguageFamily::Scala, "val_definition" | "val_declaration") => "value",
        (LanguageFamily::Scala, "var_definition" | "var_declaration") => "variable",
        (LanguageFamily::Sql, "create_function") => "function",
        (LanguageFamily::Sql, "create_index") => "index",
        (LanguageFamily::Sql, "create_materialized_view" | "create_view") => "view",
        (LanguageFamily::Sql, "create_schema") => "schema",
        (LanguageFamily::Sql, "create_sequence") => "sequence",
        (LanguageFamily::Sql, "create_table") => "table",
        (LanguageFamily::Sql, "create_trigger") => "trigger",
        (LanguageFamily::Sql, "create_type") => "type",
        (LanguageFamily::Swift, "associatedtype_declaration") => "type",
        (LanguageFamily::Swift, "class_declaration") => "type",
        (LanguageFamily::Swift, "enum_entry") => "enum_variant",
        (LanguageFamily::Swift, "function_declaration") if inside_class => "method",
        (LanguageFamily::Swift, "function_declaration") => "function",
        (LanguageFamily::Swift, "init_declaration") => "constructor",
        (LanguageFamily::Swift, "property_declaration") => "property",
        (LanguageFamily::Swift, "protocol_function_declaration") => "method",
        (LanguageFamily::Swift, "protocol_declaration") => "interface",
        (LanguageFamily::Swift, "typealias_declaration") => "type",
        (LanguageFamily::TypeScript, "class_declaration") => "class",
        (LanguageFamily::TypeScript, "interface_declaration") => "interface",
        (LanguageFamily::TypeScript, "enum_declaration") => "enum",
        (LanguageFamily::TypeScript, "function_declaration") => "function",
        (LanguageFamily::TypeScript, "method_definition") => "method",
        (LanguageFamily::TypeScript, "type_alias_declaration") => "type",
        _ => return None,
    })
}

fn declaration_symbol_kind(
    node: Node<'_>,
    family: LanguageFamily,
    inside_class: bool,
) -> Option<&'static str> {
    if typescript_function_value(node, family).is_some() {
        return Some("function");
    }
    if matches!(family, LanguageFamily::Dart)
        && node.kind() == "method_declaration"
        && let Some(signature) = node.child_by_field_name("signature")
    {
        let mut cursor = signature.walk();
        if signature
            .named_children(&mut cursor)
            .any(|child| matches!(child.kind(), "getter_signature" | "setter_signature"))
        {
            return Some("property");
        }
    }
    symbol_kind(family, node.kind(), inside_class)
}

fn typescript_function_value(node: Node<'_>, family: LanguageFamily) -> Option<Node<'_>> {
    if !matches!(family, LanguageFamily::TypeScript) || node.kind() != "variable_declarator" {
        return None;
    }
    node.child_by_field_name("value")
        .filter(|value| matches!(value.kind(), "arrow_function" | "function_expression"))
}

fn declaration_name(node: Node<'_>, family: LanguageFamily) -> Option<Node<'_>> {
    if let Some(name) = node.child_by_field_name("name") {
        return Some(name);
    }
    if matches!(family, LanguageFamily::Dart)
        && matches!(node.kind(), "function_declaration" | "method_declaration")
    {
        let signature = node.child_by_field_name("signature")?;
        if let Some(name) = signature.child_by_field_name("name") {
            return Some(name);
        }
        let mut cursor = signature.walk();
        return signature
            .named_children(&mut cursor)
            .find(|child| {
                matches!(
                    child.kind(),
                    "function_signature" | "getter_signature" | "setter_signature"
                )
            })
            .and_then(|signature| signature.child_by_field_name("name"));
    }
    if matches!(family, LanguageFamily::Dart) && node.kind() == "type_alias" {
        return first_identifier(node);
    }
    if matches!(family, LanguageFamily::Scala)
        && matches!(
            node.kind(),
            "val_definition" | "val_declaration" | "var_definition" | "var_declaration"
        )
    {
        return node
            .child_by_field_name("pattern")
            .and_then(first_identifier);
    }
    if matches!(family, LanguageFamily::C | LanguageFamily::Cpp)
        && matches!(node.kind(), "function_definition" | "function_declaration")
    {
        return node
            .child_by_field_name("declarator")
            .and_then(first_identifier);
    }
    if matches!(family, LanguageFamily::Sql) && node.kind().starts_with("create_") {
        let mut cursor = node.walk();
        if let Some(object) = node
            .named_children(&mut cursor)
            .find(|child| matches!(child.kind(), "object_reference" | "identifier"))
        {
            return first_identifier(object);
        }
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
            (
                Path::new("src/app.dart"),
                "class App { void run() {} } void main() {}",
            ),
            (Path::new("src/app.lua"), "function app.run() end"),
            (Path::new("src/app.rb"), "class App; def run; end; end"),
            (
                Path::new("src/app.scala"),
                "class App { def run(): Unit = () }",
            ),
            (Path::new("src/app.sql"), "CREATE TABLE app (id INT);"),
            (Path::new("src/app.swift"), "class App { func run() {} }"),
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
    fn extracts_typescript_arrow_function_names_and_signatures_without_bodies() {
        let source = "export function DesignCanvas() {\n  const sendCanvasRequest = async (model: ChatModel, messages: Message[]): Promise<string> => {\n    const privateBodyMarker = model.name + messages.length;\n    return privateBodyMarker;\n  };\n  return null;\n}";
        let symbols = extract_symbols(Path::new("src/DesignCanvas.tsx"), source, 16);
        let request = symbols
            .iter()
            .find(|symbol| symbol.name == "sendCanvasRequest")
            .expect("find the local arrow function");

        assert_eq!(request.kind, "function");
        assert_eq!(request.container.as_deref(), Some("DesignCanvas"));
        assert!(
            request
                .signature
                .as_deref()
                .is_some_and(|signature| signature.contains("sendCanvasRequest = async ("))
        );
        assert!(
            !request
                .signature
                .as_deref()
                .is_some_and(|signature| signature.contains("privateBodyMarker"))
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

    #[test]
    fn extracts_declaration_names_from_additional_languages() {
        let samples = [
            (
                Path::new("src/app.dart"),
                "class Counter { void increment() {} } void main() {}",
                &["Counter", "increment", "main"][..],
            ),
            (
                Path::new("src/app.lua"),
                "function Counter.increment() end",
                &["Counter.increment"][..],
            ),
            (
                Path::new("src/app.rb"),
                "class Counter; def increment; end; end",
                &["Counter", "increment"][..],
            ),
            (
                Path::new("src/app.scala"),
                "class Counter { def increment(): Unit = () }",
                &["Counter", "increment"][..],
            ),
            (
                Path::new("src/app.sql"),
                "CREATE TABLE users (id INT); CREATE VIEW active_users AS SELECT id FROM users;",
                &["users", "active_users"][..],
            ),
            (
                Path::new("src/app.swift"),
                "class Counter { func increment() {} } func main() {}",
                &["Counter", "increment", "main"][..],
            ),
        ];

        for (path, source, expected_names) in samples {
            let symbols = extract_symbols(path, source, 32);
            for expected_name in expected_names {
                assert!(
                    symbols.iter().any(|symbol| symbol.name == *expected_name),
                    "expected {expected_name} from {} but found {symbols:?}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn extracts_nested_members_from_additional_grammars() {
        let dart_symbols = extract_symbols(
            Path::new("src/model.dart"),
            "extension ModelText on String { int get size => length; }",
            16,
        );
        let size = dart_symbols
            .iter()
            .find(|symbol| symbol.name == "size")
            .expect("extract the Dart getter");
        assert_eq!(size.kind, "property");
        assert_eq!(size.container.as_deref(), Some("ModelText"));

        let scala_symbols = extract_symbols(
            Path::new("src/model.scala"),
            "object Model { val count = 1; var title = \"model\" }",
            16,
        );
        assert!(
            scala_symbols
                .iter()
                .any(|symbol| symbol.name == "count" && symbol.kind == "value")
        );
        assert!(
            scala_symbols
                .iter()
                .any(|symbol| symbol.name == "title" && symbol.kind == "variable")
        );

        let swift_symbols = extract_symbols(
            Path::new("src/model.swift"),
            "protocol Drawable { func draw() }",
            16,
        );
        let draw = swift_symbols
            .iter()
            .find(|symbol| symbol.name == "draw")
            .expect("extract the Swift protocol member");
        assert_eq!(draw.kind, "method");
        assert_eq!(draw.container.as_deref(), Some("Drawable"));
    }
}
