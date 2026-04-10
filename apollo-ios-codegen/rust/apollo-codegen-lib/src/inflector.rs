use indexmap::{IndexMap, IndexSet};
use regex::Regex;

use crate::inflection_rule::InflectionRule;

/// A compiled regex rule with its replacement pattern.
#[derive(Clone)]
struct CompiledRule {
  regex: Regex,
  replacement: String,
}

/// Converts NSRegularExpression-style replacement strings to Rust regex replacement strings.
///
/// NSRegularExpression uses `$N` where N is a single digit to refer to capture groups.
/// Rust's `regex` crate is greedy: `$1ses` is interpreted as group `1s` (or `1se`, etc.)
/// which doesn't exist. We need to convert `$N` to `${N}` when followed by alphanumeric chars.
///
/// Examples:
/// - `"$1ses"` -> `"${1}ses"`
/// - `"$1$2ves"` -> `"${1}${2}ves"`
/// - `"$1"` -> `"${1}"` (safe, though `$1` at end also works)
/// - `"s"` -> `"s"` (no groups)
/// - `""` -> `""` (empty)
fn normalize_replacement(replacement: &str) -> String {
  let mut result = String::with_capacity(replacement.len() + 4);
  let chars: Vec<char> = replacement.chars().collect();
  let mut i = 0;

  while i < chars.len() {
    if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
      // Found $N -- wrap in ${N}
      result.push('$');
      result.push('{');
      i += 1;
      // Consume the single digit (NSRegularExpression group refs are single-digit)
      result.push(chars[i]);
      result.push('}');
      i += 1;
    } else {
      result.push(chars[i]);
      i += 1;
    }
  }

  result
}

/// A string inflector that applies regex-based pluralization and singularization rules.
///
/// Mirrors InflectorKit's `TTTStringInflector` behavior:
/// - Rules added later take precedence (inserted at front of the rules list)
/// - All regex matching is case-insensitive (`(?i)` flag)
/// - Irregulars are stored bidirectionally with both lowercase and capitalized variants
/// - Uncountable check is case-insensitive (lowercased comparison)
/// - Application order: check uncountable -> check irregular -> iterate rules (first match wins)
#[derive(Clone)]
pub struct Inflector {
  plural_rules: Vec<CompiledRule>,
  singular_rules: Vec<CompiledRule>,
  irregulars_s2p: IndexMap<String, String>,
  irregulars_p2s: IndexMap<String, String>,
  uncountables: IndexSet<String>,
}

impl Default for Inflector {
  fn default() -> Self {
    Self::new()
  }
}

impl Inflector {
  /// Creates a new inflector with empty rule sets.
  pub fn new() -> Self {
    Self {
      plural_rules: Vec::new(),
      singular_rules: Vec::new(),
      irregulars_s2p: IndexMap::new(),
      irregulars_p2s: IndexMap::new(),
      uncountables: IndexSet::new(),
    }
  }

  /// Adds a rule from an `InflectionRule` enum value.
  pub fn add_rule(&mut self, rule: &InflectionRule) {
    match rule {
      InflectionRule::Pluralization {
        singular_regex,
        replacement_regex,
      } => {
        self.add_plural_rule(singular_regex, replacement_regex);
      }
      InflectionRule::Singularization {
        plural_regex,
        replacement_regex,
      } => {
        self.add_singular_rule(plural_regex, replacement_regex);
      }
      InflectionRule::Irregular { singular, plural } => {
        self.add_irregular(singular, plural);
      }
      InflectionRule::Uncountable { word } => {
        self.add_uncountable(word);
      }
    }
  }

  /// Adds a pluralization rule. The rule is inserted at the front of the rules list
  /// so that later-added rules take precedence over earlier ones.
  ///
  /// Both the pattern and replacement are removed from uncountables.
  /// The regex is compiled with case-insensitive matching (`(?i)`).
  pub fn add_plural_rule(&mut self, pattern: &str, replacement: &str) {
    self.uncountables.shift_remove(&pattern.to_lowercase());
    self.uncountables.shift_remove(&replacement.to_lowercase());

    let case_insensitive_pattern = format!("(?i){}", pattern);
    let regex = Regex::new(&case_insensitive_pattern)
      .unwrap_or_else(|e| panic!("Invalid plural regex pattern '{}': {}", pattern, e));

    self.plural_rules.insert(
      0,
      CompiledRule {
        regex,
        replacement: normalize_replacement(replacement),
      },
    );
  }

  /// Adds a singularization rule. The rule is inserted at the front of the rules list
  /// so that later-added rules take precedence over earlier ones.
  ///
  /// Both the pattern and replacement are removed from uncountables.
  /// The regex is compiled with case-insensitive matching (`(?i)`).
  pub fn add_singular_rule(&mut self, pattern: &str, replacement: &str) {
    self.uncountables.shift_remove(&pattern.to_lowercase());
    self.uncountables.shift_remove(&replacement.to_lowercase());

    let case_insensitive_pattern = format!("(?i){}", pattern);
    let regex = Regex::new(&case_insensitive_pattern)
      .unwrap_or_else(|e| panic!("Invalid singular regex pattern '{}': {}", pattern, e));

    self.singular_rules.insert(
      0,
      CompiledRule {
        regex,
        replacement: normalize_replacement(replacement),
      },
    );
  }

  /// Adds an irregular word pair. Both directions are stored (singular->plural and
  /// plural->singular), with both lowercase and capitalized variants.
  ///
  /// Both words are removed from uncountables.
  pub fn add_irregular(&mut self, singular: &str, plural: &str) {
    self.uncountables.shift_remove(&singular.to_lowercase());
    self.uncountables.shift_remove(&plural.to_lowercase());

    let s_lower = singular.to_lowercase();
    let p_lower = plural.to_lowercase();
    let s_cap = capitalize(&s_lower);
    let p_cap = capitalize(&p_lower);

    // Store lowercase variants
    self.irregulars_s2p.insert(s_lower.clone(), p_lower.clone());
    self.irregulars_p2s.insert(p_lower.clone(), s_lower.clone());

    // Store capitalized variants
    self.irregulars_s2p.insert(s_cap.clone(), p_cap.clone());
    self.irregulars_p2s.insert(p_cap, s_cap);
  }

  /// Adds an uncountable word. The word is stored in lowercase for
  /// case-insensitive comparison.
  pub fn add_uncountable(&mut self, word: &str) {
    self.uncountables.insert(word.to_lowercase());
  }

  /// Pluralizes a word using the configured rules.
  ///
  /// Application order:
  /// 1. Check if the word is uncountable (return unchanged)
  /// 2. Check if the word is an irregular singular (return irregular plural with case matching)
  /// 3. Iterate plural rules in order (first match wins)
  /// 4. If no match, return word unchanged
  pub fn pluralize(&self, word: &str) -> String {
    if word.is_empty() {
      return String::new();
    }

    // Check uncountables (case-insensitive)
    if self.uncountables.contains(&word.to_lowercase()) {
      return word.to_string();
    }

    // Check irregulars (singular -> plural)
    if let Some(irregular) = self.irregulars_s2p.get(&word.to_lowercase()) {
      return match_case(word, irregular);
    }

    // Iterate plural rules (first match wins)
    for rule in &self.plural_rules {
      if rule.regex.is_match(word) {
        return rule.regex.replace(word, &rule.replacement).to_string();
      }
    }

    word.to_string()
  }

  /// Singularizes a word using the configured rules.
  ///
  /// Application order:
  /// 1. Check if the word is uncountable (return unchanged)
  /// 2. Check if the word is an irregular plural (return irregular singular with case matching)
  /// 3. Iterate singular rules in order (first match wins)
  /// 4. If no match, return word unchanged
  pub fn singularize(&self, word: &str) -> String {
    if word.is_empty() {
      return String::new();
    }

    // Check uncountables (case-insensitive)
    if self.uncountables.contains(&word.to_lowercase()) {
      return word.to_string();
    }

    // Check irregulars (plural -> singular)
    if let Some(irregular) = self.irregulars_p2s.get(&word.to_lowercase()) {
      return match_case(word, irregular);
    }

    // Iterate singular rules (first match wins)
    for rule in &self.singular_rules {
      if rule.regex.is_match(word) {
        return rule.regex.replace(word, &rule.replacement).to_string();
      }
    }

    word.to_string()
  }
}

/// Capitalizes the first character of a string.
fn capitalize(s: &str) -> String {
  let mut chars = s.chars();
  match chars.next() {
    None => String::new(),
    Some(c) => {
      let upper: String = c.to_uppercase().collect();
      format!("{}{}", upper, chars.as_str())
    }
  }
}

/// Applies the capitalization pattern of `source` to `target`.
///
/// - If source is all uppercase: return target in all uppercase
/// - If source starts with uppercase: capitalize target
/// - Otherwise: return target as-is (lowercase)
fn match_case(source: &str, target: &str) -> String {
  if source.chars().all(|c| !c.is_alphabetic() || c.is_uppercase()) {
    target.to_uppercase()
  } else if source.starts_with(|c: char| c.is_uppercase()) {
    capitalize(target)
  } else {
    target.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_capitalize() {
    assert_eq!(capitalize("hello"), "Hello");
    assert_eq!(capitalize(""), "");
    assert_eq!(capitalize("a"), "A");
    assert_eq!(capitalize("HELLO"), "HELLO");
  }

  #[test]
  fn test_match_case_all_upper() {
    assert_eq!(match_case("CAT", "dogs"), "DOGS");
    assert_eq!(match_case("PERSON", "people"), "PEOPLE");
  }

  #[test]
  fn test_match_case_initial_cap() {
    assert_eq!(match_case("Cat", "dogs"), "Dogs");
    assert_eq!(match_case("Person", "people"), "People");
  }

  #[test]
  fn test_match_case_lower() {
    assert_eq!(match_case("cat", "dogs"), "dogs");
    assert_eq!(match_case("person", "people"), "people");
  }

  #[test]
  fn test_rule_insertion_at_front() {
    let mut inflector = Inflector::new();
    inflector.add_plural_rule("$", "s");
    inflector.add_plural_rule("s$", "ses");
    // The second rule should be at index 0
    assert_eq!(inflector.plural_rules.len(), 2);
    // "s$" was added second, so it should be first (index 0)
    assert!(inflector.plural_rules[0].regex.is_match("cats"));
  }

  #[test]
  fn test_uncountable_case_insensitive() {
    let mut inflector = Inflector::new();
    inflector.add_uncountable("sheep");
    assert_eq!(inflector.pluralize("sheep"), "sheep");
    assert_eq!(inflector.pluralize("Sheep"), "Sheep");
    assert_eq!(inflector.pluralize("SHEEP"), "SHEEP");
  }

  #[test]
  fn test_irregular_bidirectional() {
    let mut inflector = Inflector::new();
    inflector.add_irregular("person", "people");

    // Forward: singular -> plural
    assert_eq!(inflector.pluralize("person"), "people");
    assert_eq!(inflector.pluralize("Person"), "People");
    assert_eq!(inflector.pluralize("PERSON"), "PEOPLE");

    // Reverse: plural -> singular
    assert_eq!(inflector.singularize("people"), "person");
    assert_eq!(inflector.singularize("People"), "Person");
    assert_eq!(inflector.singularize("PEOPLE"), "PERSON");
  }

  #[test]
  fn test_regex_case_insensitive() {
    let mut inflector = Inflector::new();
    inflector.add_plural_rule("$", "s");

    assert_eq!(inflector.pluralize("cat"), "cats");
    assert_eq!(inflector.pluralize("Cat"), "Cats");
    // Note: for regex rules, the replacement is literal so CAT -> CATs
    // because the regex matches "$" (end of string) and appends "s"
    assert_eq!(inflector.pluralize("CAT"), "CATs");
  }

  #[test]
  fn test_empty_string() {
    let inflector = Inflector::new();
    assert_eq!(inflector.pluralize(""), "");
    assert_eq!(inflector.singularize(""), "");
  }

  #[test]
  fn test_normalize_replacement() {
    assert_eq!(normalize_replacement("$1ses"), "${1}ses");
    assert_eq!(normalize_replacement("$1$2ves"), "${1}${2}ves");
    assert_eq!(normalize_replacement("$1"), "${1}");
    assert_eq!(normalize_replacement("s"), "s");
    assert_eq!(normalize_replacement(""), "");
    assert_eq!(normalize_replacement("$1es"), "${1}es");
    assert_eq!(normalize_replacement("$1zes"), "${1}zes");
    assert_eq!(normalize_replacement("$1ices"), "${1}ices");
  }
}
