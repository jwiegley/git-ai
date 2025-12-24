//! Secret detection and redaction for prompt messages.
//!
//! This module implements entropy-based secret detection inspired by ripsecrets.
//! It identifies high-entropy strings (likely secrets/API keys) and redacts them
//! in-place before saving to git notes.

use std::collections::{HashMap, HashSet};

/// Minimum length for a string to be considered a potential secret
const MIN_SECRET_LENGTH: usize = 15;

/// Maximum length for a string to be considered a potential secret
const MAX_SECRET_LENGTH: usize = 90;

/// Number of characters to keep visible at start and end when redacting
const REDACT_VISIBLE_CHARS: usize = 4;

/// Common source code bigrams (roughly 10% of possible base64 bigrams)
/// Used to distinguish random strings from natural code/text
const BIGRAMS: &[&[u8]] = &[
    b"er", b"te", b"an", b"en", b"ma", b"ke", b"10", b"at", b"/m", b"on", b"09", b"ti", b"al",
    b"io", b".h", b"./", b"..", b"ra", b"ht", b"es", b"or", b"tm", b"pe", b"ml", b"re", b"in",
    b"3/", b"n3", b"0F", b"ok", b"ey", b"00", b"80", b"08", b"ss", b"07", b"15", b"81", b"F3",
    b"st", b"52", b"KE", b"To", b"01", b"it", b"2B", b"2C", b"/E", b"P_", b"EY", b"B7", b"se",
    b"73", b"de", b"VP", b"EV", b"to", b"od", b"B0", b"0E", b"nt", b"et", b"_P", b"A0", b"60",
    b"90", b"0A", b"ri", b"30", b"ar", b"C0", b"op", b"03", b"ec", b"ns", b"as", b"FF", b"F7",
    b"po", b"PK", b"la", b".p", b"AE", b"62", b"me", b"F4", b"71", b"8E", b"yp", b"pa", b"50",
    b"qu", b"D7", b"7D", b"rs", b"ea", b"Y_", b"t_", b"ha", b"3B", b"c/", b"D2", b"ls", b"DE",
    b"pr", b"am", b"E0", b"oc", b"06", b"li", b"do", b"id", b"05", b"51", b"40", b"ED", b"_p",
    b"70", b"ed", b"04", b"02", b"t.", b"rd", b"mp", b"20", b"d_", b"co", b"ro", b"ex", b"11",
    b"ua", b"nd", b"0C", b"0D", b"D0", b"Eq", b"le", b"EF", b"wo", b"e_", b"e.", b"ct", b"0B",
    b"_c", b"Li", b"45", b"rT", b"pt", b"14", b"61", b"Th", b"56", b"sT", b"E6", b"DF", b"nT",
    b"16", b"85", b"em", b"BF", b"9E", b"ne", b"_s", b"25", b"91", b"78", b"57", b"BE", b"ta",
    b"ng", b"cl", b"_t", b"E1", b"1F", b"y_", b"xp", b"cr", b"4F", b"si", b"s_", b"E5", b"pl",
    b"AB", b"ge", b"7E", b"F8", b"35", b"E2", b"s.", b"CF", b"58", b"32", b"2F", b"E7", b"1B",
    b"ve", b"B1", b"3D", b"nc", b"Gr", b"EB", b"C6", b"77", b"64", b"sl", b"8A", b"6A", b"_k",
    b"79", b"C8", b"88", b"ce", b"Ex", b"5C", b"28", b"EA", b"A6", b"2A", b"Ke", b"A7", b"th",
    b"CA", b"ry", b"F0", b"B6", b"7/", b"D9", b"6B", b"4D", b"DA", b"3C", b"ue", b"n7", b"9C",
    b".c", b"7B", b"72", b"ac", b"98", b"22", b"/o", b"va", b"2D", b"n.", b"_m", b"B8", b"A3",
    b"8D", b"n_", b"12", b"nE", b"ca", b"3A", b"is", b"AD", b"rt", b"r_", b"l-", b"_C", b"n1",
    b"_v", b"y.", b"yw", b"1/", b"ov", b"_n", b"_d", b"ut", b"no", b"ul", b"sa", b"CT", b"_K",
    b"SS", b"_e", b"F1", b"ty", b"ou", b"nG", b"tr", b"s/", b"il", b"na", b"iv", b"L_", b"AA",
    b"da", b"Ty", b"EC", b"ur", b"TX", b"xt", b"lu", b"No", b"r.", b"SL", b"Re", b"sw", b"_1",
    b"om", b"e/", b"Pa", b"xc", b"_g", b"_a", b"X_", b"/e", b"vi", b"ds", b"ai", b"==", b"ts",
    b"ni", b"mg", b"ic", b"o/", b"mt", b"gm", b"pk", b"d.", b"ch", b"/p", b"tu", b"sp", b"17",
    b"/c", b"ym", b"ot", b"ki", b"Te", b"FE", b"ub", b"nL", b"eL", b".k", b"if", b"he", b"34",
    b"e-", b"23", b"ze", b"rE", b"iz", b"St", b"EE", b"-p", b"be", b"In", b"ER", b"67", b"13",
    b"yn", b"ig", b"ib", b"_f", b".o", b"el", b"55", b"Un", b"21", b"fi", b"54", b"mo", b"mb",
    b"gi", b"_r", b"Qu", b"FD", b"-o", b"ie", b"fo", b"As", b"7F", b"48", b"41", b"/i", b"eS",
    b"ab", b"FB", b"1E", b"h_", b"ef", b"rr", b"rc", b"di", b"b.", b"ol", b"im", b"eg", b"ap",
    b"_l", b"Se", b"19", b"oS", b"ew", b"bs", b"Su", b"F5", b"Co", b"BC", b"ud", b"C1", b"r-",
    b"ia", b"_o", b"65", b".r", b"sk", b"o_", b"ck", b"CD", b"Am", b"9F", b"un", b"fa", b"F6",
    b"5F", b"nk", b"lo", b"ev", b"/f", b".t", b"sE", b"nO", b"a_", b"EN", b"E4", b"Di", b"AC",
    b"95", b"74", b"1_", b"1A", b"us", b"ly", b"ll", b"_b", b"SA", b"FC", b"69", b"5E", b"43",
    b"um", b"tT", b"OS", b"CE", b"87", b"7A", b"59", b"44", b"t-", b"bl", b"ad", b"Or", b"D5",
    b"A_", b"31", b"24", b"t/", b"ph", b"mm", b"f.", b"ag", b"RS", b"Of", b"It", b"FA", b"De",
    b"1D", b"/d", b"-k", b"lf", b"hr", b"gu", b"fy", b"D6", b"89", b"6F", b"4E", b"/k", b"w_",
    b"cu", b"br", b"TE", b"ST", b"R_", b"E8", b"/O",
];

/// Check if a string matches hexadecimal pattern (16+ chars of 0-9a-fA-F)
fn is_hex_string(s: &[u8]) -> bool {
    if s.len() < 16 {
        return false;
    }
    s.iter().all(|&b| b.is_ascii_hexdigit())
}

/// Check if a string matches uppercase + numbers pattern (16+ chars of 0-9A-Z)
fn is_cap_and_numbers(s: &[u8]) -> bool {
    if s.len() < 16 {
        return false;
    }
    s.iter()
        .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Calculate the probability that a string is random based on various heuristics.
///
/// Returns a value between 0 and 1, where higher values indicate
/// a higher probability of being a random/secret string.
pub fn p_random(s: &[u8]) -> f64 {
    let base = if is_hex_string(s) {
        16.0
    } else if is_cap_and_numbers(s) {
        36.0
    } else {
        64.0
    };

    let mut p = p_random_distinct_values(s, base) * p_random_char_class(s, base);

    if base == 64.0 {
        // Bigrams are only calibrated for base64
        p *= p_random_bigrams(s);
    }

    p
}

/// Calculate probability based on bigram frequency.
/// Random strings should have roughly 10% of common source code bigrams.
fn p_random_bigrams(s: &[u8]) -> f64 {
    let bigrams_set: HashSet<&[u8]> = BIGRAMS.iter().copied().collect();

    let mut num_bigrams = 0;
    for i in 0..s.len().saturating_sub(1) {
        let bigram = &s[i..=i + 1];
        if bigrams_set.contains(bigram) {
            num_bigrams += 1;
        }
    }

    p_binomial(
        s.len(),
        num_bigrams,
        (bigrams_set.len() as f64) / (64.0 * 64.0),
    )
}

/// Calculate probability based on character class distribution.
/// Looks at uppercase, lowercase, and digit ratios.
fn p_random_char_class(s: &[u8], base: f64) -> f64 {
    if base == 16.0 {
        return p_random_char_class_aux(s, b'0', b'9', 16.0);
    }

    let char_classes_36: &[(u8, u8)] = &[(b'0', b'9'), (b'A', b'Z')];
    let char_classes_64: &[(u8, u8)] = &[(b'0', b'9'), (b'A', b'Z'), (b'a', b'z')];

    let char_classes = if base == 36.0 {
        char_classes_36
    } else {
        char_classes_64
    };

    let mut min_p = f64::INFINITY;
    for (min, max) in char_classes {
        let p = p_random_char_class_aux(s, *min, *max, base);
        if p < min_p {
            min_p = p;
        }
    }

    min_p
}

fn p_random_char_class_aux(s: &[u8], min: u8, max: u8, base: f64) -> f64 {
    let mut count = 0;
    for b in s {
        if *b >= min && *b <= max {
            count += 1;
        }
    }
    let num_chars = (max - min + 1) as f64;
    p_binomial(s.len(), count, num_chars / base)
}

/// Calculate binomial probability (cumulative tail probability).
fn p_binomial(n: usize, x: usize, p: f64) -> f64 {
    let left_tail = (x as f64) < n as f64 * p;
    let min = if left_tail { 0 } else { x };
    let max = if left_tail { x } else { n };

    let mut total_p = 0.0;
    for i in min..=max {
        total_p += factorial(n) / (factorial(n - i) * factorial(i))
            * p.powi(i as i32)
            * (1.0 - p).powi((n - i) as i32);
    }

    total_p
}

/// Calculate factorial with f64 to handle large numbers.
fn factorial(n: usize) -> f64 {
    let mut res = 1.0;
    for i in 2..=n {
        res *= i as f64;
    }
    res
}

/// Calculate probability based on number of distinct character values.
/// Random strings tend to have more unique characters.
fn p_random_distinct_values(s: &[u8], base: f64) -> f64 {
    let total_possible: f64 = base.powi(s.len() as i32);
    let num_distinct_values = count_distinct_values(s);

    let mut num_more_extreme_outcomes: f64 = 0.0;
    for i in 1..=num_distinct_values {
        num_more_extreme_outcomes += num_possible_outcomes(s.len(), i, base as usize);
    }

    num_more_extreme_outcomes / total_possible
}

fn count_distinct_values(s: &[u8]) -> usize {
    let mut values_count = HashMap::<u8, usize>::new();
    for b in s {
        *values_count.entry(*b).or_insert(0) += 1;
    }
    values_count.len()
}

fn num_possible_outcomes(num_values: usize, num_distinct_values: usize, base: usize) -> f64 {
    let mut res = base as f64;
    for i in 1..num_distinct_values {
        res *= (base - i) as f64;
    }
    res *= num_distinct_configurations(num_values, num_distinct_values);
    res
}

/// Calculate number of distinct configurations using memoization.
fn num_distinct_configurations(num_values: usize, num_distinct_values: usize) -> f64 {
    if num_distinct_values == 1 || num_distinct_values == num_values {
        return 1.0;
    }

    // Use a simple cache instead of the memoize crate
    let mut cache: HashMap<(usize, usize, usize), f64> = HashMap::new();
    num_distinct_configurations_aux(
        num_distinct_values,
        0,
        num_values - num_distinct_values,
        &mut cache,
    )
}

fn num_distinct_configurations_aux(
    num_positions: usize,
    position: usize,
    remaining_values: usize,
    cache: &mut HashMap<(usize, usize, usize), f64>,
) -> f64 {
    if remaining_values == 0 {
        return 1.0;
    }

    let key = (num_positions, position, remaining_values);
    if let Some(&cached) = cache.get(&key) {
        return cached;
    }

    let mut num_configs = 0.0;
    if position + 1 < num_positions {
        num_configs +=
            num_distinct_configurations_aux(num_positions, position + 1, remaining_values, cache);
    }
    num_configs += (position + 1) as f64
        * num_distinct_configurations_aux(num_positions, position, remaining_values - 1, cache);

    cache.insert(key, num_configs);
    num_configs
}

/// Check if a string is likely a random/secret string.
/// Returns true if the string appears to be a secret.
pub fn is_random(s: &[u8]) -> bool {
    let p = p_random(s);

    if p < 1.0 / 1e5 {
        return false;
    }

    // If no digits, require higher probability threshold
    let contains_num = s.iter().any(|&b| b.is_ascii_digit());
    if !contains_num && p < 1.0 / 1e4 {
        return false;
    }

    true
}

/// Check if a byte is a valid secret character (alphanumeric or common secret chars).
/// Excludes `=` which is typically a delimiter (e.g., KEY=value).
/// Note: `=` at the end of base64 strings is handled specially.
fn is_secret_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'_' | b'-' | b'.' | b'~')
}

/// Extract potential secret tokens from text.
/// Returns a vector of (start_index, token) pairs.
pub fn extract_tokens(text: &str) -> Vec<(usize, String)> {
    let mut tokens = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Skip non-secret characters
        if !is_secret_char(bytes[i]) {
            i += 1;
            continue;
        }

        // Found start of potential token
        let start = i;
        while i < bytes.len() && is_secret_char(bytes[i]) {
            i += 1;
        }

        let token = &text[start..i];
        let len = token.len();

        // Only consider tokens in the right length range
        if len >= MIN_SECRET_LENGTH && len <= MAX_SECRET_LENGTH {
            tokens.push((start, token.to_string()));
        }
    }

    tokens
}

/// Redact a secret string, keeping first and last few characters visible.
/// Format: "sk_live_abc123" -> "sk_l********c123"
pub fn redact_secret(secret: &str) -> String {
    let len = secret.len();
    if len <= REDACT_VISIBLE_CHARS * 2 {
        // Too short to meaningfully redact
        return "*".repeat(len);
    }

    let prefix = &secret[..REDACT_VISIBLE_CHARS];
    let suffix = &secret[len - REDACT_VISIBLE_CHARS..];
    format!("{}********{}", prefix, suffix)
}

/// Redact all detected secrets in a text string.
/// Returns a tuple of (redacted_text, redaction_count).
pub fn redact_secrets_in_text(text: &str) -> (String, usize) {
    let tokens = extract_tokens(text);

    // Filter to only actual secrets
    let secrets: Vec<(usize, String)> = tokens
        .into_iter()
        .filter(|(_, token)| is_random(token.as_bytes()))
        .collect();

    let count = secrets.len();

    if secrets.is_empty() {
        return (text.to_string(), 0);
    }

    // Replace secrets from end to start to preserve indices
    let mut result = text.to_string();
    for (start, secret) in secrets.into_iter().rev() {
        let redacted = redact_secret(&secret);
        result.replace_range(start..start + secret.len(), &redacted);
    }

    (result, count)
}

use crate::authorship::authorship_log::PromptRecord;
use crate::authorship::transcript::Message;
use std::collections::BTreeMap;

/// Redact secrets from all prompt messages using entropy-based detection.
/// Scans user and assistant message text for high-entropy strings (API keys,
/// passwords, tokens) and replaces them with partially masked versions.
/// Returns the total number of secrets redacted.
pub fn redact_secrets_from_prompts(prompts: &mut BTreeMap<String, PromptRecord>) -> usize {
    let mut total_redactions = 0;
    for record in prompts.values_mut() {
        for message in &mut record.messages {
            match message {
                Message::User { text, .. } | Message::Assistant { text, .. } => {
                    let (redacted, count) = redact_secrets_in_text(text);
                    *text = redacted;
                    total_redactions += count;
                }
                Message::ToolUse { .. } => {
                    // Skip tool use messages - they contain structured data
                }
            }
        }
    }
    total_redactions
}

/// Strip all messages from prompts (used when sharing is disabled).
pub fn strip_prompt_messages(prompts: &mut BTreeMap<String, PromptRecord>) {
    for record in prompts.values_mut() {
        record.messages.clear();
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn test_p_random_random_strings() {
        // These should be detected as random
        assert!(p_random(b"pk_test_TYooMQauvdEDq54NiTphI7jx") > 1.0 / 1e4);
        assert!(p_random(b"sk_test_4eC39HqLyjWDarjtT1zdp7dc") > 1.0 / 1e4);
    }

    #[test]
    fn test_p_random_non_random_strings() {
        // These should NOT be detected as random
        assert!(p_random(b"hello_world") < 1.0 / 1e6);
        assert!(p_random(b"PROJECT_NAME_ALIAS") < 1.0 / 1e4);
    }

    #[test]
    fn test_is_random() {
        // Secrets
        assert!(is_random(b"pk_test_TYooMQauvdEDq54NiTphI7jx"));
        assert!(is_random(b"sk_test_4eC39HqLyjWDarjtT1zdp7dc"));
        assert!(is_random(b"AKIAIOSFODNN7EXAMPLE"));

        // Not secrets
        assert!(!is_random(b"hello_world"));
        assert!(!is_random(b"my_variable_name"));
    }

    #[test]
    fn test_extract_tokens() {
        let text = "API_KEY=sk_test_4eC39HqLyjWDarjtT1zdp7dc";
        let tokens = extract_tokens(text);
        assert!(!tokens.is_empty());
        // The token should be extracted (API_KEY is 7 chars, too short; the secret is 32 chars)
        assert!(
            tokens
                .iter()
                .any(|(_, t)| t == "sk_test_4eC39HqLyjWDarjtT1zdp7dc")
        );
    }

    #[test]
    fn test_redact_secret() {
        assert_eq!(
            redact_secret("sk_test_4eC39HqLyjWDarjtT1zdp7dc"),
            "sk_t********p7dc"
        );
        assert_eq!(redact_secret("AKIAIOSFODNN7EXAMPLE"), "AKIA********MPLE");
        assert_eq!(redact_secret("short"), "*****"); // Too short
    }

    #[test]
    fn test_redact_secrets_in_text() {
        let text = "Set API_KEY=sk_test_4eC39HqLyjWDarjtT1zdp7dc in your config";
        let (redacted, count) = redact_secrets_in_text(text);
        assert!(!redacted.contains("sk_test_4eC39HqLyjWDarjtT1zdp7dc"));
        assert!(redacted.contains("sk_t********p7dc"));
        assert_eq!(count, 1);
    }

    #[test]
    fn test_no_redaction_for_normal_text() {
        let text = "This is normal text without any secrets";
        let (redacted, count) = redact_secrets_in_text(text);
        assert_eq!(text, redacted);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_distinct_values() {
        assert_eq!(count_distinct_values(b"abca"), 3);
        assert_eq!(count_distinct_values(b"aaaaaa"), 1);
        assert_eq!(count_distinct_values(b"abcdef"), 6);
    }

    #[test]
    fn test_redact_secret_in_lorem_ipsum() {
        let text = r#"
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor 
incididunt ut labore et dolore magna aliqua. Here is my API key: 
sk_live_51HG8vDKj2xPmVnRqT9wYzABC and you should use it carefully.
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut 
aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in 
voluptate velit esse cillum dolore eu fugiat nulla pariatur.
"#;
        let (redacted, count) = redact_secrets_in_text(text);

        // Secret should be redacted
        assert!(!redacted.contains("sk_live_51HG8vDKj2xPmVnRqT9wYzABC"));
        assert!(redacted.contains("sk_l********zABC"));
        assert_eq!(count, 1);

        // Rest of text should be intact
        assert!(redacted.contains("Lorem ipsum dolor sit amet"));
        assert!(redacted.contains("consectetur adipiscing elit"));
        assert!(redacted.contains("Here is my API key:"));
    }

    #[test]
    fn test_redact_multiple_secrets_in_code() {
        let code = r#"
use std::env;

fn main() {
    // Database credentials
    let db_password = "xK9mP2nQ7rS4tU6vW8yZ1aB3cD5eF7gH";
    
    // API configuration
    let stripe_key = "sk_test_4eC39HqLyjWDarjtT1zdp7dc";
    let aws_key = "AKIAIOSFODNN7EXAMPLE";
    
    // Normal config values - should NOT be redacted
    let app_name = "my_application_name";
    let log_level = "debug";
    let max_connections = 100;
    
    println!("Starting application...");
}
"#;
        let (redacted, count) = redact_secrets_in_text(code);

        // Secrets should be redacted
        assert!(!redacted.contains("xK9mP2nQ7rS4tU6vW8yZ1aB3cD5eF7gH"));
        assert!(!redacted.contains("sk_test_4eC39HqLyjWDarjtT1zdp7dc"));
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert_eq!(count, 3);

        // Normal identifiers should remain
        assert!(redacted.contains("my_application_name"));
        assert!(redacted.contains("debug"));
        assert!(redacted.contains("max_connections"));
        assert!(redacted.contains("println!"));
    }

    #[test]
    fn test_redact_secret_in_json_config() {
        let json = r#"{
    "database": {
        "host": "localhost",
        "port": 5432,
        "password": "Rj7kL9mN2pQ4sT6vX8zA1bC3dE5fG7hI"
    },
    "api": {
        "endpoint": "https://api.example.com",
        "key": "pk_live_TYooMQauvdEDq54NiTphI7jx"
    },
    "logging": {
        "level": "info",
        "format": "json"
    }
}"#;
        let (redacted, count) = redact_secrets_in_text(json);

        // Secrets should be redacted
        assert!(!redacted.contains("Rj7kL9mN2pQ4sT6vX8zA1bC3dE5fG7hI"));
        assert!(!redacted.contains("pk_live_TYooMQauvdEDq54NiTphI7jx"));
        assert_eq!(count, 2);

        // Normal config should remain
        assert!(redacted.contains("localhost"));
        assert!(redacted.contains("5432"));
        assert!(redacted.contains("https://api.example.com"));
        assert!(redacted.contains("info"));
    }

    #[test]
    fn test_redact_secret_in_env_file() {
        let env_content = r#"
# Application configuration
APP_NAME=my-cool-app
DEBUG=true
LOG_LEVEL=debug

# Secrets - these should be redacted
DATABASE_URL=postgres://user:pA5sW0rD9xK2mN7qR4tU6vY8zA1bC3dE@localhost:5432/mydb
STRIPE_SECRET_KEY=sk_live_51HG8vDKj2xPmVnRqT9wYzABC
AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
JWT_SECRET=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9

# More normal config
PORT=3000
HOST=0.0.0.0
"#;
        let (redacted, count) = redact_secrets_in_text(env_content);

        println!("redacted: {}", redacted);
        assert_debug_snapshot!(redacted);
        // Secrets should be redacted
        assert!(!redacted.contains("pA5sW0rD9xK2mN7qR4tU6vY8zA1bC3dE"));
        assert!(!redacted.contains("sk_live_51HG8vDKj2xPmVnRqT9wYzABC"));
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(count >= 3); // At least 3 secrets

        // Normal values should remain
        assert!(redacted.contains("my-cool-app"));
        assert!(redacted.contains("DEBUG=true"));
        assert!(redacted.contains("PORT=3000"));
    }

    #[test]
    fn test_no_false_positives_in_normal_code() {
        let code = r#"
pub fn calculate_total(items: &[Item]) -> f64 {
    items.iter().map(|item| item.price * item.quantity as f64).sum()
}

struct Configuration {
    database_host: String,
    database_port: u16,
    application_name: String,
    max_retry_attempts: u32,
}

impl Configuration {
    pub fn from_environment() -> Self {
        Self {
            database_host: std::env::var("DB_HOST").unwrap_or_default(),
            database_port: 5432,
            application_name: "my_service".to_string(),
            max_retry_attempts: 3,
        }
    }
}
"#;
        let (redacted, count) = redact_secrets_in_text(code);

        // Code should be completely unchanged - no false positives
        assert_eq!(code, redacted);
        assert_eq!(count, 0);
    }
}
