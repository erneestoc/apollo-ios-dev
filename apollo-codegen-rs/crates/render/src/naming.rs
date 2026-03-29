//! Naming and formatting utilities for Swift code generation.

use apollo_codegen_config::types::InflectionRule;
use regex::Regex;

/// Type names that conflict with Swift/ApolloAPI built-in types.
/// When a nested selection set struct would have one of these names,
/// it gets suffixed with `_SelectionSet` to avoid the collision.
/// Matches Swift's `SwiftKeywords.TypeNamesToSuffix`.
const TYPE_NAMES_TO_SUFFIX: &[&str] = &[
    "Any", "DataDict", "DocumentType", "Fragments", "FragmentContainer",
    "ParentType", "Protocol", "Schema", "Selection", "Self", "String",
    "Bool", "Int", "Float", "Double", "ID", "Type", "Error", "_",
];

/// If a selection set struct name conflicts with Swift built-ins,
/// suffix it with `_SelectionSet` to disambiguate.
pub fn as_selection_set_name(name: &str) -> String {
    if TYPE_NAMES_TO_SUFFIX.contains(&name) {
        format!("{}_SelectionSet", name)
    } else {
        name.to_string()
    }
}

/// Disambiguate a schema type name with a type-specific suffix.
/// Matches Swift's `GraphQLNamedType.render(as: .typename)` behavior.
pub fn as_schema_type_name(name: &str, suffix: &str) -> String {
    let uppercased = first_uppercased(name);
    if TYPE_NAMES_TO_SUFFIX.contains(&uppercased.as_str()) {
        format!("{}{}", uppercased, suffix)
    } else {
        uppercased
    }
}

/// If a fragment name conflicts with Swift built-ins,
/// suffix it with `_Fragment` to disambiguate.
pub fn as_fragment_name(name: &str) -> String {
    let uppercased = first_uppercased(name);
    if TYPE_NAMES_TO_SUFFIX.contains(&uppercased.as_str()) {
        format!("{}_Fragment", uppercased)
    } else {
        uppercased
    }
}

/// Swift keywords that must be escaped with backticks for field accessors and enum cases.
/// Matches Swift CLI's `SwiftKeywords.FieldAccessorNamesToEscape`.
const SWIFT_FIELD_ACCESSOR_KEYWORDS: &[&str] = &[
    "associatedtype", "class", "deinit", "enum", "extension", "fileprivate", "func",
    "import", "init", "inout", "internal", "let", "operator", "private", "precedencegroup",
    "protocol", "Protocol", "public", "rethrows", "static", "struct", "subscript", "typealias",
    "var", "break", "case", "catch", "continue", "default", "defer", "do", "else",
    "fallthrough", "for", "guard", "if", "in", "repeat", "return", "throw", "switch",
    "where", "while", "as", "false", "is", "nil", "self", "Self", "super", "throws",
    "true", "try", "_",
];

/// Escape a name if it's a Swift keyword that requires backtick escaping.
/// Uses the same keyword list as Swift CLI's `FieldAccessorNamesToEscape`.
pub fn escape_swift_name(name: &str) -> String {
    if SWIFT_FIELD_ACCESSOR_KEYWORDS.contains(&name) {
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
/// Convert a GraphQL enum value name to Swift camelCase.
///
/// Matches Swift's `String.convertToCamelCase()` exactly:
/// 1. **Has underscores?** Split by `_`, `.capitalized` each segment
///    (capitalize first char, lowercase rest), rejoin, then `firstLowercased`.
/// 2. **No underscores, ALL uppercase?** Lowercase everything.
/// 3. **No underscores, has lowercase?** Only lowercase the first cased character
///    (preserve all other casing). This is Swift's `.firstLowercased`.
///
/// Examples:
///   "FyRFJCm33OKsz80" → "fyRFJCm33OKsz80"  (case 3: firstLowercased)
///   "ALL_CAPS_VALUE"   → "allCapsValue"       (case 1: split + capitalize + firstLower)
///   "PENDING"          → "pending"             (case 2: all uppercase → all lowercase)
///   "camelCase"        → "camelCase"           (case 3: already starts lowercase)
///   "adP6xzG5CDa6_90" → "adp6xzg5cda690"     (case 1: has underscore)
pub fn to_camel_case(s: &str) -> String {
    let has_underscore = s.contains('_');

    if !has_underscore {
        // No underscores — check if ALL uppercase or has lowercase
        let has_lowercase = s.chars().any(|c| c.is_lowercase());
        if has_lowercase {
            // Case 3: just firstLowercased (lowercase only the first cased character)
            first_lowercased(s)
        } else {
            // Case 2: ALL uppercase → all lowercase
            s.to_lowercase()
        }
    } else {
        // Case 1: has underscores → split, .capitalized each segment, rejoin, firstLowercased.
        // Foundation's .capitalized treats digit-to-letter transitions as word boundaries:
        //   "abc123DEF" → "Abc123Def" (capitalize after digits)
        //   "9UQ1" → "9Uq1" (capitalize first letter after leading digits)
        let joined: String = s
            .split('_')
            .map(|segment| {
                if segment.is_empty() {
                    "_".to_string()
                } else {
                    foundation_capitalized(segment)
                }
            })
            .collect();
        first_lowercased(&joined)
    }
}

/// Emulate Foundation's `.capitalized` property on String.
///
/// Word boundaries are: start of string, after any non-letter character (digits, symbols).
/// At each word boundary, the first letter is uppercased and subsequent letters are lowercased
/// until the next boundary.
fn foundation_capitalized(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut at_word_start = true;

    for c in s.chars() {
        if c.is_alphabetic() {
            if at_word_start {
                result.extend(c.to_uppercase());
                at_word_start = false;
            } else {
                result.extend(c.to_lowercase());
            }
        } else {
            // Non-letter (digit, symbol, etc.) → next letter starts a new word
            result.push(c);
            at_word_start = true;
        }
    }
    result
}

/// InflectorKit-compatible English singularization for GraphQL field names.
///
/// Uses the same regex-based singularization rules as the Swift codegen
/// (InflectorKit's StringInflector). Rules are applied in reverse order
/// (last matching rule wins).
///
/// Examples:
/// - `allAnimals` → `allAnimal`
/// - `classroomPets` → `classroomPet`
/// - `predators` → `predator`
/// - `height` → `height` (unchanged, not plural)
/// - `Data` → `Datum` (Latin: `([ti])a$` → `$1um`)
pub fn singularize(s: &str) -> String {
    use regex::Regex;

    if s.is_empty() {
        return s.to_string();
    }

    // Uncountable words (never singularize)
    let uncountables = [
        "equipment", "information", "rice", "money", "species",
        "series", "fish", "sheep", "jeans", "police",
    ];
    let lower = s.to_lowercase();
    for word in &uncountables {
        if lower == *word {
            return s.to_string();
        }
    }

    // Irregular words
    let irregulars = [
        ("people", "person"),
        ("men", "man"),
        ("children", "child"),
        ("sexes", "sex"),
        ("moves", "move"),
        ("zombies", "zombie"),
    ];
    for (plural, singular) in &irregulars {
        if lower == *plural {
            // Preserve original casing of first character
            let first = s.chars().next().unwrap();
            return format!("{}{}", first, &singular[1..]);
        }
    }

    // Singularization rules (applied in reverse order — last match wins)
    // These match InflectorKit's default singularization rules exactly.
    let rules: Vec<(&str, &str)> = vec![
        ("s$", ""),
        ("(ss)$", "$1"),
        ("(n)ews$", "${1}ews"),
        ("([ti])a$", "${1}um"),
        ("((a)naly|(b)a|(d)iagno|(p)arenthe|(p)rogno|(s)ynop|(t)he)(sis|ses)$", "${1}sis"),
        ("(^analy)(sis|ses)$", "${1}sis"),
        ("([^f])ves$", "${1}fe"),
        ("(hive)s$", "$1"),
        ("(tive)s$", "$1"),
        ("([lr])ves$", "${1}f"),
        ("([^aeiouy]|qu)ies$", "${1}y"),
        ("(s)eries$", "${1}eries"),
        ("(m)ovies$", "${1}ovie"),
        ("(x|ch|ss|sh)es$", "$1"),
        ("^(m|l)ice$", "${1}ouse"),
        ("(bus)(es)?$", "$1"),
        ("(o)es$", "$1"),
        ("(shoe)s$", "$1"),
        ("(cris|test)(is|es)$", "${1}is"),
        ("^(a)x[ie]s$", "${1}xis"),
        ("(octop|vir)(us|i)$", "${1}us"),
        ("(alias|status)(es)?$", "$1"),
        ("^(ox)en", "$1"),
        ("(vert|ind)ices$", "${1}ex"),
        ("(matr)ices$", "${1}ix"),
        ("(quiz)zes$", "$1"),
        ("(database)s$", "$1"),
    ];

    // Apply rules in reverse (last matching rule wins, per InflectorKit behavior)
    for &(pattern, replacement) in rules.iter().rev() {
        let re = Regex::new(&format!("(?i){}", pattern)).unwrap();
        if re.is_match(s) {
            return re.replace(s, replacement).to_string();
        }
    }

    s.to_string()
}

/// A pluralizer/singularizer that mirrors Swift's InflectorKit-based `Pluralizer`.
///
/// Uses regex-based rules applied in reverse order (last added rule wins).
/// Supports custom rules from `additionalInflectionRules` config.
pub struct Pluralizer {
    singular_rules: Vec<(Regex, String)>,
    irregular_singular_to_plural: Vec<(String, String)>,
    uncountable: Vec<String>,
}

impl Pluralizer {
    /// Create a new Pluralizer with default rules and optional custom rules.
    pub fn new(custom_rules: &[InflectionRule]) -> Self {
        let mut p = Pluralizer {
            singular_rules: Vec::new(),
            irregular_singular_to_plural: Vec::new(),
            uncountable: Vec::new(),
        };

        // Add default rules (matching Swift's Pluralizer.defaultRules)
        p.add_default_rules();

        // Add custom rules (applied after defaults, so they take priority)
        for rule in custom_rules {
            match rule {
                InflectionRule::Pluralization { .. } => {
                    // Pluralization rules are not used for singularization
                }
                InflectionRule::Singularization { plural_regex, replacement_regex } => {
                    if let Ok(re) = Regex::new(plural_regex) {
                        p.singular_rules.push((re, replacement_regex.clone()));
                    }
                }
                InflectionRule::Irregular { singular, plural } => {
                    p.irregular_singular_to_plural.push((singular.clone(), plural.clone()));
                }
                InflectionRule::Uncountable { word } => {
                    p.uncountable.push(word.to_lowercase());
                }
            }
        }

        p
    }

    fn add_default_rules(&mut self) {
        // Default singularization rules (from Swift's Pluralizer.defaultRules)
        // Applied in order; last matching rule wins (we search in reverse).
        let singular_rules = vec![
            ("s$", ""),
            ("(ss)$", "$1"),
            ("(n)ews$", "$1ews"),
            ("([ti])a$", "$1um"),
            ("((a)naly|(b)a|(d)iagno|(p)arenthe|(p)rogno|(s)ynop|(t)he)(sis|ses)$", "$1sis"),
            ("(^analy)(sis|ses)$$", "$1sis"),
            ("([^f])ves$", "$1fe"),
            ("(hive)s$", "$1"),
            ("(tive)s$", "$1"),
            ("([lr])ves$", "$1f"),
            ("([^aeiouy]|qu)ies$", "$1y"),
            ("(s)eries$", "$1eries"),
            ("(m)ovies$", "$1ovie"),
            ("(x|ch|ss|sh)es$", "$1"),
            ("^(m|l)ice$", "$1ouse"),
            ("(bus)(es)?$", "$1"),
            ("(o)es$", "$1"),
            ("(shoe)s$", "$1"),
            ("(cris|test)(is|es)$", "$1is"),
            ("^(a)x[ie]s$", "$1xis"),
            ("(octop|vir)(us|i)$", "$1us"),
            ("(alias|status)(es)?$", "$1"),
            ("^(ox)en", "$1"),
            ("(vert|ind)ices$", "$1ex"),
            ("(matr)ices$", "$1ix"),
            ("(quiz)zes$", "$1"),
            ("(database)s$", "$1"),
        ];

        for (pattern, replacement) in singular_rules {
            if let Ok(re) = Regex::new(pattern) {
                self.singular_rules.push((re, replacement.to_string()));
            }
        }

        // Default irregular words
        let irregulars = vec![
            ("person", "people"),
            ("man", "men"),
            ("child", "children"),
            ("sex", "sexes"),
            ("move", "moves"),
            ("zombie", "zombies"),
        ];
        for (singular, plural) in irregulars {
            self.irregular_singular_to_plural.push((singular.to_string(), plural.to_string()));
        }

        // Default uncountable words
        let uncountable = vec![
            "equipment", "information", "rice", "money", "species",
            "series", "fish", "sheep", "jeans", "police",
        ];
        for word in uncountable {
            self.uncountable.push(word.to_string());
        }
    }

    /// Singularize a string using the configured rules.
    ///
    /// Checks uncountable words and irregular pairs first, then applies
    /// regex rules in reverse order (last added rule wins).
    pub fn singularize(&self, s: &str) -> String {
        if s.is_empty() {
            return s.to_string();
        }

        let lower = s.to_lowercase();

        // Check uncountable
        if self.uncountable.contains(&lower) {
            return s.to_string();
        }

        // Check irregular (plural → singular)
        for (singular, plural) in self.irregular_singular_to_plural.iter().rev() {
            if lower == plural.to_lowercase() {
                // Preserve the capitalization of the original string
                if s.chars().next().unwrap_or(' ').is_uppercase() {
                    return first_uppercased(singular);
                }
                return singular.clone();
            }
        }

        // Apply regex rules in reverse order (last rule has priority)
        for (regex, replacement) in self.singular_rules.iter().rev() {
            if regex.is_match(s) {
                return regex.replace(s, replacement.as_str()).to_string();
            }
        }

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
