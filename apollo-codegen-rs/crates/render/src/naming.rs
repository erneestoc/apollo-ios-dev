//! Naming and formatting utilities for Swift code generation.

/// Swift reserved words that must be escaped with backticks.
const SWIFT_RESERVED_WORDS: &[&str] = &[
    "Any", "Protocol", "Self", "Type", "actor", "as", "associatedtype", "associativity",
    "async", "await", "break", "case", "catch", "class", "continue", "convenience",
    "default", "defer", "deinit", "didSet", "do", "dynamic", "else", "enum",
    "extension", "fallthrough", "false", "fileprivate", "final", "for", "func",
    "get", "guard", "if", "import", "in", "indirect", "infix", "init", "inout",
    "internal", "is", "lazy", "left", "let", "mutating", "nil", "none",
    "nonmutating", "open", "operator", "optional", "override", "postfix", "precedence",
    "precedencegroup", "prefix", "private", "protocol", "public", "repeat",
    "required", "rethrows", "return", "right", "safe", "self", "set", "some",
    "static", "struct", "subscript", "super", "switch", "throw", "throws",
    "true", "try", "typealias", "unowned", "unsafe", "var", "weak", "where",
    "while", "willSet",
];

/// Escape a name if it's a Swift reserved word.
pub fn escape_swift_name(name: &str) -> String {
    if SWIFT_RESERVED_WORDS.contains(&name) {
        format!("`{}`", name)
    } else {
        name.to_string()
    }
}

/// Convert the first character to uppercase.
pub fn first_uppercased(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Convert the first character to lowercase.
pub fn first_lowercased(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

/// Convert a SCREAMING_SNAKE_CASE or snake_case name to camelCase.
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    let mut first = true;

    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_uppercase().next().unwrap_or(c));
            capitalize_next = false;
        } else if first {
            result.push(c.to_lowercase().next().unwrap_or(c));
            first = false;
        } else {
            result.push(c.to_lowercase().next().unwrap_or(c));
        }
    }

    result
}

/// Basic English singularization for GraphQL field names.
///
/// The Swift codegen singularizes list field names when creating struct names.
/// For example:
/// - `allAnimals` → `AllAnimal`
/// - `classroomPets` → `ClassroomPet`
/// - `predators` → `Predator`
/// - `height` → `Height` (unchanged, not plural)
pub fn singularize(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }

    // Only singularize if it ends with 's' and is likely plural
    if let Some(stripped) = s.strip_suffix('s') {
        if stripped.is_empty() {
            return s.to_string();
        }
        // Don't singularize words ending in "ss" (e.g., "class", "address")
        if stripped.ends_with('s') {
            return s.to_string();
        }
        // Don't singularize words ending in "us" (e.g., "status", "radius")
        if stripped.ends_with('u') {
            return s.to_string();
        }
        // Handle "ies" → "y" (e.g., "buddies" → "buddy")
        if let Some(base) = stripped.strip_suffix("ie") {
            return format!("{}y", base);
        }
        // Handle "ves" → "f" (e.g., "wolves" → "wolf") -- "wolves" stripped to "wolve"
        // Actually after stripping 's', "wolves" → "wolve", check for "ve" → "f"
        // Handle "es" ending (after stripping 's', we have the base ending in 'e')
        // e.g., "classes" → "classe" -- but we already handled "ss" above
        // "boxes" → "boxe" -- need to handle "xes", "shes", "ches"
        if stripped.ends_with('e') {
            // Check for "xes" → "x", "shes" → "sh", "ches" → "ch"
            let without_e = &stripped[..stripped.len() - 1];
            if without_e.ends_with('x')
                || without_e.ends_with("sh")
                || without_e.ends_with("ch")
                || without_e.ends_with('z')
            {
                return without_e.to_string();
            }
            // "ves" → "f" (e.g., "wolves" → "wolve" → "wolf")
            if without_e.ends_with('v') {
                let without_ve = &without_e[..without_e.len() - 1];
                return format!("{}f", without_ve);
            }
        }
        stripped.to_string()
    } else {
        s.to_string()
    }
}

/// Render a GraphQL type name as a Swift typename.
pub fn render_as_typename(name: &str) -> String {
    first_uppercased(name)
}

/// Render a GraphQL type name escaped if needed.
pub fn render_typename_escaped(name: &str) -> String {
    escape_swift_name(&first_uppercased(name))
}
