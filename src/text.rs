//! Turning what a player types into something comparable: accents, articles, plurals and small typos.

use unicode_normalization::UnicodeNormalization;

use crate::settings::Language;

/// "L'Éléphant  " -> "elephant" (French), "The Hammer!" -> "hammer" (English).
pub fn normalize(language: Language, text: &str) -> String {
    let folded: String = text
        .nfd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .map(|character| match character {
            'œ' => 'o',
            'æ' => 'a',
            character if character.is_alphanumeric() => character,
            _ => ' ',
        })
        .collect();
    let words: Vec<&str> = folded.split_whitespace().collect();
    let skip = leading_articles(language, &words);
    words[skip..].join(" ")
}

/// How many leading words are articles; never strips the whole answer.
fn leading_articles(language: Language, words: &[&str]) -> usize {
    let articles: &[&[&str]] = match language {
        Language::En => &[&["the"], &["a"], &["an"]],
        Language::Fr => &[&["de", "la"], &["de", "l"], &["le"], &["la"], &["les"], &["l"], &["un"], &["une"], &["des"], &["du"]],
        Language::Nl => &[&["de"], &["het"], &["een"], &["t"]],
    };
    articles
        .iter()
        .find(|article| words.len() > article.len() && words.starts_with(article))
        .map_or(0, |article| article.len())
}

/// The normalized text plus simple singular forms, so "hammers" matches "hammer".
pub fn variants(language: Language, normalized: &str) -> Vec<String> {
    let mut forms = vec![normalized.to_owned()];
    let mut add = |form: String| {
        if form.chars().count() >= 3 && !forms.contains(&form) {
            forms.push(form);
        }
    };
    match language {
        Language::En => {
            if let Some(stem) = normalized.strip_suffix("ies") {
                add(format!("{stem}y"));
            }
            if let Some(stem) = normalized.strip_suffix("es") {
                add(stem.to_owned());
            }
            if let Some(stem) = normalized.strip_suffix('s') {
                add(stem.to_owned());
            }
        }
        Language::Fr => {
            if let Some(stem) = normalized.strip_suffix(['s', 'x']) {
                add(stem.to_owned());
            }
        }
        Language::Nl => {
            for suffix in ["en", "s"] {
                if let Some(stem) = normalized.strip_suffix(suffix) {
                    add(stem.to_owned());
                }
            }
        }
    }
    forms
}

/// How many typos to forgive: none for short words, one from 5 letters, two from 9.
pub fn typo_allowance(normalized: &str) -> usize {
    match normalized.chars().count() {
        0..=4 => 0,
        5..=8 => 1,
        _ => 2,
    }
}

pub fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1; right.len() + 1];
        for (column, right_char) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left_char != *right_char);
            current[column + 1] = substitution.min(previous[column + 1] + 1).min(current[column] + 1);
        }
        previous = current;
    }
    previous[right.len()]
}

/// Content words of a prompt, for spotting near-duplicates ("Something in a toolbox" vs "Things found in toolboxes").
pub fn keywords(language: Language, text: &str) -> Vec<String> {
    let stop: &[&str] = match language {
        Language::En => &["a", "an", "the", "that", "you", "can", "in", "on", "of", "something", "things", "thing", "name", "with", "for", "is", "are", "be", "to", "or", "and", "your", "find", "found", "would", "could", "might", "usually", "often", "people", "someone"],
        Language::Fr => &["un", "une", "le", "la", "les", "des", "de", "du", "que", "qui", "on", "en", "dans", "quelque", "chose", "choses", "avec", "pour", "est", "sont", "et", "ou", "ton", "ta", "tes", "trouve", "trouver", "souvent"],
        Language::Nl => &["een", "de", "het", "dat", "die", "je", "in", "op", "van", "iets", "dingen", "ding", "met", "voor", "is", "zijn", "en", "of", "jouw", "kan", "vind", "vindt", "gevonden", "vaak"],
    };
    let mut words: Vec<String> = normalize(language, text)
        .split_whitespace()
        .filter(|word| word.len() > 2 && !stop.contains(word))
        // The shortest singular form is the word's stem: "toolboxes" -> "toolbox".
        .filter_map(|word| variants(language, word).into_iter().min_by_key(String::len))
        .collect();
    words.sort();
    words.dedup();
    words
}

/// Share of words two prompts have in common (0 = nothing, 1 = the same words).
pub fn similarity(left: &[String], right: &[String]) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let shared = left.iter().filter(|word| right.contains(word)).count();
    shared as f64 / (left.len() + right.len() - shared) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizing_strips_accents_articles_and_punctuation() {
        assert_eq!(normalize(Language::En, "  The Hammer! "), "hammer");
        assert_eq!(normalize(Language::Fr, "L'Éléphant"), "elephant");
        assert_eq!(normalize(Language::Fr, "de la crème"), "creme");
        assert_eq!(normalize(Language::Nl, "het Fietswiel"), "fietswiel");
        assert_eq!(normalize(Language::En, "the"), "the");
        assert_eq!(normalize(Language::En, "plumb-bob"), "plumb bob");
    }

    #[test]
    fn variants_include_singulars() {
        assert!(variants(Language::En, "berries").contains(&"berry".to_owned()));
        assert!(variants(Language::En, "hammers").contains(&"hammer".to_owned()));
        assert!(variants(Language::Fr, "marteaux").contains(&"marteau".to_owned()));
        assert!(variants(Language::Nl, "hamers").contains(&"hamer".to_owned()));
    }

    #[test]
    fn edit_distance_counts_single_typos() {
        assert_eq!(edit_distance("screwdriver", "screwdrvier"), 2);
        assert_eq!(edit_distance("hammer", "hamer"), 1);
        assert_eq!(edit_distance("same", "same"), 0);
    }

    #[test]
    fn near_duplicate_prompts_are_similar() {
        let left = keywords(Language::En, "Something you'd find in a toolbox");
        let right = keywords(Language::En, "Things found in toolboxes");
        assert!(similarity(&left, &right) >= 0.5, "{left:?} vs {right:?}");
        let other = keywords(Language::En, "A fruit that isn't round");
        assert!(similarity(&left, &other) < 0.2);
    }
}
