use std::collections::HashSet;
use std::sync::LazyLock;

static BANNED_WORDS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let content = include_str!("../assets/comment_banned_words.txt");
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(normalize)
        .collect()
});

const LEET: &[(char, &[char])] = &[
    ('a', &['4', '@']),
    ('b', &['8']),
    ('c', &['(']),
    ('e', &['3']),
    ('g', &['9']),
    ('i', &['1', '!', '|']),
    ('l', &['1', '|']),
    ('o', &['0']),
    ('s', &['5', '$']),
    ('t', &['7', '+']),
    ('z', &['2']),
];

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn normalize_with_leet(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        let lower = c.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            result.push(lower);
        } else {
            let mut substituted = false;
            for (canonical, variants) in LEET {
                if variants.contains(&c) || c == *canonical {
                    result.push(*canonical);
                    substituted = true;
                    break;
                }
            }
            if !substituted && c.is_ascii() {
                // skip non-alphanumeric ASCII
            }
        }
    }
    result
}

pub fn contains_banned_content(content: &str) -> bool {
    if content.is_empty() {
        return false;
    }
    let normalized = normalize(content);
    let normalized_leet = normalize_with_leet(content);
    for word in BANNED_WORDS.iter() {
        if normalized.contains(word.as_str()) || normalized_leet.contains(word.as_str()) {
            return true;
        }
    }
    false
}
