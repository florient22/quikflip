//! Writing tomorrow's Rarefind with Claude, and judging contested answers. The code decides the categories and
//! checks every result against the database; Claude only supplies the creative part.

use std::collections::{HashMap, HashSet};

use anyhow::Context;
use chrono::{Days, NaiveDate};
use rand::seq::SliceRandom;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{PROMPT_COUNT, TIER_POINTS, has_puzzle};
use crate::claude::Claude;
use crate::day::format_day;
use crate::settings::Language;
use crate::text;

/// Prompt topics. The least recently used ones are picked each day, so every topic comes around.
pub const CATEGORIES: &[&str] = &[
    "animals", "birds", "insects and small creatures", "sea life", "food", "fruit and vegetables", "desserts and sweets",
    "drinks", "in the kitchen", "around the house", "tools and DIY", "clothing and accessories", "sports",
    "games and toys", "music", "musical instruments", "films and TV", "books and stories", "jobs", "school",
    "science", "space", "weather", "plants and flowers", "nature and landscapes", "geography", "cities and buildings",
    "transport", "the human body", "health", "technology", "holidays and celebrations", "colours and shapes",
    "feelings", "history", "myths and legends", "art", "words and language", "things with a special property",
    "things you can do", "everyday objects", "camping and outdoors",
];
const RECENT_DAYS: u64 = 90;
const MIN_ANSWERS: usize = 25;
const MIN_DISTINCT_TIERS: usize = 3;
const MAX_ATTEMPTS: usize = 3;
const DUPLICATE_SIMILARITY: f64 = 0.6;
const MAX_PROMPT_CHARS: usize = 140;
const CONTEST_SAMPLE_ANSWERS: usize = 40;

#[derive(Debug, Deserialize)]
struct Generated {
    prompts: Vec<GeneratedPrompt>,
}

#[derive(Debug, Deserialize)]
struct GeneratedPrompt {
    category: String,
    prompt: String,
    answers: Vec<GeneratedAnswer>,
}

#[derive(Debug, Deserialize)]
struct GeneratedAnswer {
    answer: String,
    #[serde(default)]
    aliases: Vec<String>,
    tier: u8,
}

#[derive(Debug)]
pub struct PuzzlePrompt {
    pub category: String,
    pub prompt: String,
    /// (normalized key, shown answer, tier); aliases are extra keys pointing to the same shown answer.
    pub answers: Vec<(String, String, u8)>,
}

fn language_name(language: Language) -> &'static str {
    match language {
        Language::En => "English",
        Language::Fr => "French (standard French; Belgian usage is welcome)",
        Language::Nl => "Dutch (standard Dutch; Belgian/Flemish usage is welcome)",
    }
}

fn pick_categories(connection: &Connection, language: Language) -> anyhow::Result<Vec<&'static str>> {
    let mut statement = connection.prepare("SELECT category, MAX(day) FROM rarefind_prompts WHERE lang = ?1 GROUP BY category")?;
    let last_used: HashMap<String, String> = statement
        .query_map(params![language.code()], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut categories: Vec<&'static str> = CATEGORIES.to_vec();
    categories.shuffle(&mut rand::rng());
    // Never used first, then the ones that have rested longest; the shuffle breaks ties.
    categories.sort_by_key(|category| last_used.get(*category).cloned());
    categories.truncate(PROMPT_COUNT);
    Ok(categories)
}

fn recent_prompts(connection: &Connection, language: Language, day: NaiveDate) -> anyhow::Result<Vec<String>> {
    let since = format_day(day - Days::new(RECENT_DAYS));
    let mut statement = connection.prepare("SELECT prompt FROM rarefind_prompts WHERE lang = ?1 AND day >= ?2 ORDER BY day DESC")?;
    let prompts = statement.query_map(params![language.code(), since], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(prompts)
}

pub fn puzzle_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["prompts"],
        "properties": {
            "prompts": {
                "type": "array", "minItems": PROMPT_COUNT, "maxItems": PROMPT_COUNT,
                "items": {
                    "type": "object", "additionalProperties": false,
                    "required": ["category", "prompt", "answers"],
                    "properties": {
                        "category": { "type": "string" },
                        "prompt": { "type": "string" },
                        "answers": {
                            "type": "array",
                            "items": {
                                "type": "object", "additionalProperties": false,
                                "required": ["answer", "aliases", "tier"],
                                "properties": {
                                    "answer": { "type": "string" },
                                    "aliases": { "type": "array", "items": { "type": "string" } },
                                    "tier": { "type": "integer", "minimum": 1, "maximum": 5 }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

pub fn puzzle_prompt(language: Language, categories: &[&str], recent: &[String], feedback: &[String]) -> String {
    let mut prompt = format!(
        "You write the daily puzzle for Rarefind, a word game. The player gets {PROMPT_COUNT} prompts and about 25 seconds per prompt \
         to type ONE answer that fits. Every valid answer scores, but rarer valid answers score more, so the fun is in thinking of \
         something unusual that still clearly fits.\n\n\
         Language: write every prompt and every answer in {}.\n\n\
         Categories, one prompt each, in this order: {}.\n\n\
         Prompts:\n\
         - Open-ended, with at least 40 correct answers, answerable in 1 to 3 words.\n\
         - Clear and unambiguous, fun, family-friendly; no trick questions, no current events, no answers that need a date or number.\n\
         - Vary the style (\"something you find in…\", \"an animal that…\", \"a word that rhymes with…\", \"a job where…\").\n\n\
         Answers, for every prompt:\n\
         - List as many correct answers as you can: aim for 50 to 80, from the most obvious to obscure-but-clearly-correct.\n\
         - Write each answer the way a player would type it: lowercase, singular, no article.\n\
         - Put common synonyms and spelling variants in aliases.\n\
         - tier is rarity: 1 = what most people say first, 2 = common, 3 = less common, 4 = rare, 5 = very rare but clearly correct. \
           Roughly 15% tier 1, 25% tier 2, 25% tier 3, 20% tier 4, 15% tier 5.\n",
        language_name(language),
        categories.join("; ")
    );
    if !recent.is_empty() {
        prompt.push_str("\nThese prompts were used recently. Do not reuse them or write close variations of them:\n");
        for previous in recent {
            prompt.push_str(&format!("- {previous}\n"));
        }
    }
    if !feedback.is_empty() {
        prompt.push_str("\nYour previous attempt was rejected for these reasons; fix them:\n");
        for issue in feedback {
            prompt.push_str(&format!("- {issue}\n"));
        }
    }
    prompt
}

/// Checks Claude's puzzle: shape, answer counts, tier spread and near-duplicates. Returns the issues to send back.
fn validate(language: Language, generated: Generated, recent: &[String]) -> Result<Vec<PuzzlePrompt>, Vec<String>> {
    let mut issues = Vec::new();
    if generated.prompts.len() != PROMPT_COUNT {
        issues.push(format!("There must be exactly {PROMPT_COUNT} prompts, not {}.", generated.prompts.len()));
    }
    let recent_keywords: Vec<(Vec<String>, &String)> = recent.iter().map(|text| (text::keywords(language, text), text)).collect();
    let mut accepted_keywords: Vec<Vec<String>> = Vec::new();
    let mut prompts = Vec::new();
    for (index, item) in generated.prompts.into_iter().enumerate() {
        let number = index + 1;
        let prompt_text = item.prompt.trim().to_owned();
        if prompt_text.is_empty() || prompt_text.chars().count() > MAX_PROMPT_CHARS {
            issues.push(format!("Prompt {number} must have between 1 and {MAX_PROMPT_CHARS} characters."));
        }
        let keywords = text::keywords(language, &prompt_text);
        if let Some((_, previous)) = recent_keywords.iter().find(|(words, _)| text::similarity(&keywords, words) >= DUPLICATE_SIMILARITY) {
            issues.push(format!("Prompt {number} (\"{prompt_text}\") is too close to the earlier prompt \"{previous}\"."));
        }
        if accepted_keywords.iter().any(|words| text::similarity(&keywords, words) >= DUPLICATE_SIMILARITY) {
            issues.push(format!("Prompt {number} (\"{prompt_text}\") is too close to another prompt in this puzzle."));
        }
        accepted_keywords.push(keywords);

        let mut seen = HashSet::new();
        let mut answers = Vec::new();
        let mut distinct = 0;
        let mut tiers = HashSet::new();
        for answer in item.answers {
            if !(1..=5).contains(&answer.tier) {
                continue;
            }
            let display = answer.answer.trim().to_lowercase();
            let key = text::normalize(language, &display);
            if key.is_empty() || !seen.insert(key.clone()) {
                continue;
            }
            distinct += 1;
            tiers.insert(answer.tier);
            answers.push((key, display.clone(), answer.tier));
            for alias in answer.aliases {
                let alias_key = text::normalize(language, &alias);
                if !alias_key.is_empty() && seen.insert(alias_key.clone()) {
                    answers.push((alias_key, display.clone(), answer.tier));
                }
            }
        }
        if distinct < MIN_ANSWERS {
            issues.push(format!("Prompt {number} has only {distinct} usable answers; it needs at least {MIN_ANSWERS} (aim for 50 or more)."));
        }
        if tiers.len() < MIN_DISTINCT_TIERS || !tiers.iter().any(|tier| *tier >= 4) {
            issues.push(format!("Prompt {number} needs answers across the tiers, including rare ones (tier 4 or 5)."));
        }
        prompts.push(PuzzlePrompt { category: item.category.trim().to_owned(), prompt: prompt_text, answers });
    }
    if issues.is_empty() { Ok(prompts) } else { Err(issues) }
}

fn store(connection: &mut Connection, day: &str, language: Language, prompts: &[PuzzlePrompt]) -> anyhow::Result<()> {
    let transaction = connection.transaction()?;
    for (position, prompt) in prompts.iter().enumerate() {
        transaction.execute(
            "INSERT INTO rarefind_prompts (day, lang, position, category, prompt) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![day, language.code(), position as i64, prompt.category, prompt.prompt],
        )?;
        for (key, display, tier) in &prompt.answers {
            transaction.execute(
                "INSERT INTO rarefind_answers (day, lang, position, answer, display, tier) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![day, language.code(), position as i64, key, display, tier],
            )?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// Generates and stores the puzzle for one day and language, retrying with feedback when validation fails.
pub async fn generate(claude: &Claude, connection: &mut Connection, day: NaiveDate, language: Language) -> anyhow::Result<()> {
    let day_text = format_day(day);
    if has_puzzle(connection, &day_text, language)? {
        return Ok(());
    }
    let categories = pick_categories(connection, language)?;
    let recent = recent_prompts(connection, language, day)?;
    let mut feedback = Vec::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let prompt = puzzle_prompt(language, &categories, &recent, &feedback);
        let generated: Generated = claude.structured(&prompt, &puzzle_schema()).await.context("asking Claude for the puzzle")?;
        match validate(language, generated, &recent) {
            Ok(prompts) => {
                store(connection, &day_text, language, &prompts)?;
                return Ok(());
            }
            Err(issues) => {
                tracing::warn!("Rarefind {day_text} {}: attempt {attempt} rejected: {}", language.code(), issues.join(" | "));
                feedback = issues;
            }
        }
    }
    anyhow::bail!("Claude's puzzle was rejected {MAX_ATTEMPTS} times; last issues: {}", feedback.join(" | "))
}

#[derive(Debug, Deserialize)]
struct Verdicts {
    verdicts: Vec<Verdict>,
}

#[derive(Debug, Deserialize)]
struct Verdict {
    id: usize,
    valid: bool,
    tier: u8,
    reason: String,
}

struct PendingContest {
    day: String,
    language: Language,
    position: i64,
    typed: String,
    prompt: String,
}

fn pending_contests(connection: &Connection) -> anyhow::Result<Vec<PendingContest>> {
    let mut statement = connection.prepare(
        "SELECT c.day, c.lang, c.position, c.typed, p.prompt FROM rarefind_contests c
         JOIN rarefind_prompts p ON p.day = c.day AND p.lang = c.lang AND p.position = c.position
         WHERE c.status = 'pending' ORDER BY c.day",
    )?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
        .collect::<Result<Vec<(String, String, i64, String, String)>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(day, lang, position, typed, prompt)| {
            Language::from_code(&lang).map(|language| PendingContest { day, language, position, typed, prompt })
        })
        .collect())
}

fn sample_answers(connection: &Connection, contest: &PendingContest) -> anyhow::Result<Vec<(String, u8)>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT display, tier FROM rarefind_answers WHERE day = ?1 AND lang = ?2 AND position = ?3 ORDER BY tier, display LIMIT ?4",
    )?;
    let answers = statement
        .query_map(params![contest.day, contest.language.code(), contest.position, CONTEST_SAMPLE_ANSWERS as i64], |row| {
            Ok((row.get(0)?, row.get::<_, i64>(1)? as u8))
        })?
        .collect::<Result<_, _>>()?;
    Ok(answers)
}

fn verdict_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false, "required": ["verdicts"],
        "properties": { "verdicts": { "type": "array", "items": {
            "type": "object", "additionalProperties": false, "required": ["id", "valid", "tier", "reason"],
            "properties": {
                "id": { "type": "integer" }, "valid": { "type": "boolean" },
                "tier": { "type": "integer", "minimum": 1, "maximum": 5 }, "reason": { "type": "string" }
            }
        } } }
    })
}

/// Applies one verdict: an accepted answer joins the answer list and the play keeps the better of the two scores.
fn apply_verdict(connection: &mut Connection, contest: &PendingContest, verdict: &Verdict) -> anyhow::Result<()> {
    let lang = contest.language.code();
    let transaction = connection.transaction()?;
    if verdict.valid {
        let tier = verdict.tier.clamp(1, 5);
        let points = TIER_POINTS[usize::from(tier) - 1];
        let key = text::normalize(contest.language, &contest.typed);
        let display = contest.typed.trim().to_lowercase();
        transaction.execute(
            "INSERT INTO rarefind_answers (day, lang, position, answer, display, tier) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT DO NOTHING",
            params![contest.day, lang, contest.position, key, display, tier],
        )?;
        transaction.execute(
            "UPDATE rarefind_plays SET outcome = 'answered', answer = ?1, tier = ?2, points = ?3
             WHERE day = ?4 AND lang = ?5 AND position = ?6 AND outcome != 'playing' AND points < ?3",
            params![display, tier, points, contest.day, lang, contest.position],
        )?;
        transaction.execute(
            "UPDATE rarefind_contests SET status = 'accepted', tier = ?1, reason = ?2 WHERE day = ?3 AND lang = ?4 AND position = ?5",
            params![tier, verdict.reason, contest.day, lang, contest.position],
        )?;
    } else {
        transaction.execute(
            "UPDATE rarefind_contests SET status = 'rejected', reason = ?1 WHERE day = ?2 AND lang = ?3 AND position = ?4",
            params![verdict.reason, contest.day, lang, contest.position],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

/// Judges every pending contest in one Claude call. Returns how many were decided.
pub async fn judge_contests(claude: &Claude, connection: &mut Connection) -> anyhow::Result<usize> {
    let contests = pending_contests(connection)?;
    if contests.is_empty() {
        return Ok(0);
    }
    let mut prompt = String::from(
        "You judge contested answers in Rarefind, a word game where players type one answer that fits a prompt. \
         Each item below was rejected because it was not on the answer list. Decide fairly whether it is a correct answer to \
         the prompt (in the item's language; minor spelling slips are fine). If valid, give its rarity tier compared to the \
         listed answers: 1 = what most people say first ... 5 = very rare but clearly correct. Keep each reason to one short sentence.\n",
    );
    for (id, contest) in contests.iter().enumerate() {
        let answers = sample_answers(connection, contest)?;
        let listed: Vec<String> = answers.iter().map(|(answer, tier)| format!("{answer} ({tier})")).collect();
        prompt.push_str(&format!(
            "\nItem {id}\nLanguage: {}\nPrompt: {}\nListed answers (tier): {}\nContested answer: {}\n",
            language_name(contest.language),
            contest.prompt,
            listed.join(", "),
            contest.typed
        ));
    }
    let verdicts: Verdicts = claude.structured(&prompt, &verdict_schema()).await.context("asking Claude to judge contests")?;
    let mut decided = 0;
    for verdict in &verdicts.verdicts {
        if let Some(contest) = contests.get(verdict.id) {
            apply_verdict(connection, contest, verdict)?;
            decided += 1;
        }
    }
    Ok(decided)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated(prompts: Vec<(&str, usize)>) -> Generated {
        Generated {
            prompts: prompts
                .into_iter()
                .map(|(prompt, count)| GeneratedPrompt {
                    category: "tools".into(),
                    prompt: prompt.into(),
                    answers: (0..count)
                        .map(|index| GeneratedAnswer { answer: format!("answer {index}"), aliases: vec![], tier: (index % 5) as u8 + 1 })
                        .collect(),
                })
                .collect(),
        }
    }

    const PROMPTS: [&str; 7] = [
        "Something you'd find in a toolbox",
        "An animal with stripes",
        "A word that rhymes with blue",
        "A job where you wear a uniform",
        "Something that floats",
        "A dessert served cold",
        "A famous river",
    ];

    #[test]
    fn a_good_puzzle_passes() {
        let prompts = validate(Language::En, generated(PROMPTS.iter().map(|prompt| (*prompt, 40)).collect()), &[]).unwrap();
        assert_eq!(prompts.len(), PROMPT_COUNT);
    }

    #[test]
    fn thin_answer_lists_and_repeats_are_rejected() {
        let mut input: Vec<(&str, usize)> = PROMPTS.iter().map(|prompt| (*prompt, 40)).collect();
        input[1].1 = 10;
        let recent = vec!["Things found in toolboxes".to_owned()];
        let issues = validate(Language::En, generated(input), &recent).unwrap_err();
        assert!(issues.iter().any(|issue| issue.contains("Prompt 2 has only 10")), "{issues:?}");
        assert!(issues.iter().any(|issue| issue.contains("too close to the earlier prompt")), "{issues:?}");
    }

    #[test]
    fn categories_rotate_to_the_least_recently_used() {
        let connection = super::super::tests::connection_with_puzzle("2026-11-08");
        let picked = pick_categories(&connection, Language::En).unwrap();
        assert_eq!(picked.len(), PROMPT_COUNT);
        assert!(!picked.contains(&"tools and DIY"), "a category used yesterday should rest: {picked:?}");
    }
}
