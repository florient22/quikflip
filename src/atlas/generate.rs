//! Writing Atlas Hunt with Claude. The code plans the day (data clues with computed answers, plus themes for clues
//! from general knowledge) and checks every result; Claude words the clues, rates rarity and writes fun facts.

use std::collections::{HashMap, HashSet};

use anyhow::Context;
use chrono::{Days, NaiveDate};
use rand::seq::SliceRandom;
use rusqlite::{Connection, params};
use serde::Deserialize;
use serde_json::{Value, json};

use super::CLUE_COUNT;
use super::clues::{self, DataClue, MIN_VALID};
use super::countries;
use crate::claude::Claude;
use crate::day::format_day;
use crate::rarefind::generate::language_name;
use crate::settings::{AtlasRegion, Language};
use crate::text;

const DATA_CLUES_PER_DAY: usize = 3;
/// A data clue (for example "borders Germany") rests this long before it can come back.
const COOLDOWN_DAYS: u64 = 60;
const RECENT_DAYS: u64 = 90;
const MAX_ATTEMPTS: usize = 3;
const MAX_CLUE_CHARS: usize = 120;
const MAX_THEME_VALID: usize = 45;
const DUPLICATE_SIMILARITY: f64 = 0.6;
/// Claude may forget to rate a few countries of a data clue; those get the middle tier.
const MAX_UNRATED_SHARE: f64 = 0.25;
const DEFAULT_TIER: u8 = 3;
/// Keep at least this many unseen fun facts for every country that can be found in the coming days.
const MIN_UNSEEN_FACTS: i64 = 2;
const NEW_FACTS_PER_COUNTRY: usize = 3;
const FACT_BATCH: usize = 25;
const MIN_FACT_CHARS: usize = 20;
const MAX_FACT_CHARS: usize = 220;
const THEME_PREFIX: &str = "theme:";

/// Themes for clues that need general knowledge. The least recently used theme goes first.
pub const THEMES: &[&str] = &[
    "flags (colours, symbols, shapes)",
    "sport (Olympic hosts, World Cups, national sports)",
    "food and drink",
    "history (empires, independence, famous events)",
    "nature (mountains, rivers, deserts, volcanoes, wildlife)",
    "famous landmarks and wonders",
    "culture (music, festivals, film, art, traditions)",
    "inventions, famous companies and products",
    "geography records and features (seas, coastlines, islands, the equator)",
    "famous people (scientists, explorers, artists)",
];

enum Plan {
    Data(DataClue),
    Theme(&'static str),
}

impl Plan {
    fn kind(&self) -> String {
        match self {
            Plan::Data(clue) => clue.kind.clone(),
            Plan::Theme(theme) => format!("{THEME_PREFIX}{theme}"),
        }
    }
}

/// The family of a data clue ("borders", "language"…), so one day never has two clues of the same family.
fn family(kind: &str) -> &str {
    kind.split(':').next().unwrap_or(kind)
}

fn recent_kinds(connection: &Connection, language: Language, day: NaiveDate) -> anyhow::Result<HashMap<String, String>> {
    let since = format_day(day - Days::new(COOLDOWN_DAYS.max(RECENT_DAYS)));
    let mut statement = connection.prepare("SELECT kind, MAX(day) FROM atlas_clues WHERE lang = ?1 AND day >= ?2 GROUP BY kind")?;
    let kinds = statement.query_map(params![language.code(), since], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<_, _>>()?;
    Ok(kinds)
}

fn recent_clues(connection: &Connection, language: Language, day: NaiveDate) -> anyhow::Result<Vec<String>> {
    let since = format_day(day - Days::new(RECENT_DAYS));
    let mut statement = connection.prepare("SELECT clue FROM atlas_clues WHERE lang = ?1 AND day >= ?2 ORDER BY day DESC")?;
    let clues = statement.query_map(params![language.code(), since], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(clues)
}

fn plan(connection: &Connection, day: NaiveDate, language: Language, region: AtlasRegion) -> anyhow::Result<Vec<Plan>> {
    let used = recent_kinds(connection, language, day)?;
    let cooldown_start = format_day(day - Days::new(COOLDOWN_DAYS));
    let mut candidates: Vec<DataClue> = clues::candidates(region, language)
        .into_iter()
        .filter(|clue| used.get(&clue.kind).is_none_or(|last| *last < cooldown_start))
        .collect();
    let mut rng = rand::rng();
    candidates.shuffle(&mut rng);
    let mut families = HashSet::new();
    let mut plans: Vec<Plan> = Vec::new();
    for clue in candidates {
        if plans.len() == DATA_CLUES_PER_DAY {
            break;
        }
        if families.insert(family(&clue.kind).to_owned()) {
            plans.push(Plan::Data(clue));
        }
    }
    let mut themes: Vec<&'static str> = THEMES.to_vec();
    themes.shuffle(&mut rng);
    // Never used first, then the ones that have rested longest; the shuffle breaks ties.
    themes.sort_by_key(|theme| used.get(&format!("{THEME_PREFIX}{theme}")).cloned());
    plans.extend(themes.into_iter().take(CLUE_COUNT - plans.len()).map(Plan::Theme));
    plans.shuffle(&mut rng);
    Ok(plans)
}

fn region_label(region: AtlasRegion) -> &'static str {
    region.dataset_name().unwrap_or("the whole world")
}

fn country_line(code: &str) -> String {
    countries::by_code(code).map_or_else(|| code.to_owned(), |country| format!("{code} {}", country.names.en))
}

fn puzzle_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false, "required": ["clues"],
        "properties": { "clues": {
            "type": "array", "minItems": CLUE_COUNT, "maxItems": CLUE_COUNT,
            "items": {
                "type": "object", "additionalProperties": false, "required": ["number", "clue", "countries"],
                "properties": {
                    "number": { "type": "integer", "minimum": 1, "maximum": CLUE_COUNT },
                    "clue": { "type": "string" },
                    "countries": { "type": "array", "items": {
                        "type": "object", "additionalProperties": false, "required": ["code", "tier"],
                        "properties": { "code": { "type": "string" }, "tier": { "type": "integer", "minimum": 1, "maximum": 5 } }
                    } }
                }
            }
        } }
    })
}

fn puzzle_prompt(language: Language, region: AtlasRegion, plans: &[Plan], recent: &[String], feedback: &[String]) -> String {
    let mut prompt = format!(
        "You write today's Atlas Hunt, a geography game. The player gets {CLUE_COUNT} clues and picks ONE country per clue on a globe. \
         Every country that fits scores, but rarer picks score more, so the fun is in finding a country few people would think of.\n\n\
         Language: write every clue in {}. Keep each clue short (under {MAX_CLUE_CHARS} characters), clear and fun. \
         Write it as a description of the country, for example \"Shares a border with Germany\".\n\
         Region: {}. Only countries in this region count.\n\n\
         Tier = how likely a typical player is to think of that country for that clue: 1 = the first ones most people name, \
         2 = common, 3 = less common, 4 = rare, 5 = almost nobody thinks of it. Use the whole scale.\n\n",
        language_name(language),
        region_label(region)
    );
    let mut theme_plans = false;
    for (index, plan) in plans.iter().enumerate() {
        let number = index + 1;
        match plan {
            Plan::Data(clue) => {
                prompt.push_str(&format!(
                    "Clue {number}: a fact from our data. Meaning: \"{}\". These are exactly the countries that fit; rate every one of them \
                     and add no others: {}.\n",
                    clue.meaning,
                    clue.countries.iter().map(|code| country_line(code)).collect::<Vec<_>>().join(", ")
                ));
            }
            Plan::Theme(theme) => {
                theme_plans = true;
                prompt.push_str(&format!(
                    "Clue {number}: invent a clue on the theme \"{theme}\". It must be clear-cut and verifiable (no opinions, no current events, \
                     nothing that changes from year to year), and between {MIN_VALID} and {MAX_THEME_VALID} countries in the region must fit. \
                     List EVERY country that fits, using the codes below, and rate each one.\n"
                ));
            }
        }
    }
    if theme_plans {
        let list: Vec<String> = countries::in_region(region).map(|country| country_line(&country.code)).collect();
        prompt.push_str(&format!("\nCountry codes for the invented clues: {}.\n", list.join(", ")));
    }
    if !recent.is_empty() {
        prompt.push_str("\nThese clues were used recently. Do not reuse them or write close variations of them:\n");
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

#[derive(Debug, Deserialize)]
struct Generated {
    clues: Vec<GeneratedClue>,
}

#[derive(Debug, Deserialize)]
struct GeneratedClue {
    number: usize,
    clue: String,
    countries: Vec<GeneratedCountry>,
}

#[derive(Debug, Deserialize)]
struct GeneratedCountry {
    code: String,
    tier: u8,
}

#[derive(Debug)]
struct PuzzleClue {
    kind: String,
    clue: String,
    answers: Vec<Rated>,
}

/// A country code with its rarity tier.
type Rated = (String, u8);

fn distinct_tiers(answers: &[Rated]) -> usize {
    answers.iter().map(|(_, tier)| tier).collect::<HashSet<_>>().len()
}

/// Checks Claude's clues against the plan and the country data. Returns the issues to send back.
fn validate(language: Language, region: AtlasRegion, plans: &[Plan], generated: Generated, recent: &[String]) -> Result<Vec<PuzzleClue>, Vec<String>> {
    let mut issues = Vec::new();
    let mut by_number: HashMap<usize, GeneratedClue> = generated.clues.into_iter().map(|clue| (clue.number, clue)).collect();
    let recent_keywords: Vec<(Vec<String>, &String)> = recent.iter().map(|clue| (text::keywords(language, clue), clue)).collect();
    let mut seen_keywords: Vec<Vec<String>> = Vec::new();
    let mut puzzle = Vec::new();
    for (index, plan) in plans.iter().enumerate() {
        let number = index + 1;
        let Some(item) = by_number.remove(&number) else {
            issues.push(format!("Clue {number} is missing."));
            continue;
        };
        let clue_text = item.clue.trim().to_owned();
        if clue_text.is_empty() || clue_text.chars().count() > MAX_CLUE_CHARS {
            issues.push(format!("Clue {number} must have between 1 and {MAX_CLUE_CHARS} characters."));
        }
        let keywords = text::keywords(language, &clue_text);
        // Data clues are kept apart by their cooldown ("starts with B" and "starts with M" share most words but differ);
        // only invented clues are checked for repeats by wording.
        if matches!(plan, Plan::Theme(_)) {
            if let Some((_, previous)) = recent_keywords.iter().find(|(words, _)| text::similarity(&keywords, words) >= DUPLICATE_SIMILARITY) {
                issues.push(format!("Clue {number} (\"{clue_text}\") is too close to the earlier clue \"{previous}\"."));
            }
            if seen_keywords.iter().any(|words| text::similarity(&keywords, words) >= DUPLICATE_SIMILARITY) {
                issues.push(format!("Clue {number} (\"{clue_text}\") is too close to another clue in this puzzle."));
            }
        }
        seen_keywords.push(keywords);
        let rated: HashMap<String, u8> =
            item.countries.into_iter().filter(|country| (1..=5).contains(&country.tier)).map(|country| (country.code.trim().to_uppercase(), country.tier)).collect();
        let answers: Vec<Rated> = match plan {
            Plan::Data(clue) => {
                let unrated = clue.countries.iter().filter(|code| !rated.contains_key(**code)).count();
                if unrated as f64 > clue.countries.len() as f64 * MAX_UNRATED_SHARE {
                    issues.push(format!("Clue {number}: rate every listed country ({unrated} of {} have no tier).", clue.countries.len()));
                }
                clue.countries.iter().map(|code| ((*code).to_owned(), rated.get(*code).copied().unwrap_or(DEFAULT_TIER))).collect()
            }
            Plan::Theme(_) => {
                let (mut answers, unknown): (Vec<Rated>, Vec<Rated>) =
                    rated.into_iter().partition(|(code, _)| countries::by_code(code).is_some_and(|country| country.in_region(region)));
                if !unknown.is_empty() {
                    issues.push(format!(
                        "Clue {number} uses codes that are not in the country list for this region: {}.",
                        unknown.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>().join(", ")
                    ));
                }
                if !(MIN_VALID..=MAX_THEME_VALID).contains(&answers.len()) {
                    issues.push(format!("Clue {number} has {} countries; it needs between {MIN_VALID} and {MAX_THEME_VALID}.", answers.len()));
                }
                answers.sort();
                answers
            }
        };
        let needed_tiers = if answers.len() >= 6 { 3 } else { 2 };
        if distinct_tiers(&answers) < needed_tiers {
            issues.push(format!("Clue {number}: spread the countries over at least {needed_tiers} different tiers."));
        }
        puzzle.push(PuzzleClue { kind: plan.kind(), clue: clue_text, answers });
    }
    if issues.is_empty() { Ok(puzzle) } else { Err(issues) }
}

fn store(connection: &mut Connection, day: &str, language: Language, region: AtlasRegion, puzzle: &[PuzzleClue]) -> anyhow::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute("INSERT INTO atlas_days (day, lang, region) VALUES (?1, ?2, ?3)", params![day, language.code(), region.code()])?;
    for (position, clue) in puzzle.iter().enumerate() {
        transaction.execute(
            "INSERT INTO atlas_clues (day, lang, position, kind, clue) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![day, language.code(), position as i64, clue.kind, clue.clue],
        )?;
        for (country, tier) in &clue.answers {
            transaction.execute(
                "INSERT INTO atlas_answers (day, lang, position, country, tier) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![day, language.code(), position as i64, country, tier],
            )?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// Later puzzles made for another region and not played yet are removed, so a new region applies from tomorrow.
pub fn drop_other_regions(connection: &Connection, today: NaiveDate, language: Language, region: AtlasRegion) -> anyhow::Result<usize> {
    let removed = connection.execute(
        "DELETE FROM atlas_days WHERE lang = ?1 AND day > ?2 AND region != ?3
           AND NOT EXISTS (SELECT 1 FROM atlas_plays p WHERE p.day = atlas_days.day AND p.lang = atlas_days.lang)",
        params![language.code(), format_day(today), region.code()],
    )?;
    Ok(removed)
}

/// Generates and stores the puzzle for one day and language, retrying with feedback when validation fails.
pub async fn generate(claude: &Claude, connection: &mut Connection, day: NaiveDate, language: Language, region: AtlasRegion) -> anyhow::Result<()> {
    let day_text = format_day(day);
    if super::day_region(connection, &day_text, language)?.is_some() {
        return Ok(());
    }
    let plans = plan(connection, day, language, region)?;
    let recent = recent_clues(connection, language, day)?;
    let mut feedback = Vec::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let prompt = puzzle_prompt(language, region, &plans, &recent, &feedback);
        let generated: Generated = claude.structured(&prompt, &puzzle_schema()).await.context("asking Claude for the clues")?;
        match validate(language, region, &plans, generated, &recent) {
            Ok(puzzle) => {
                store(connection, &day_text, language, region, &puzzle)?;
                return Ok(());
            }
            Err(issues) => {
                tracing::warn!("Atlas Hunt {day_text} {}: attempt {attempt} rejected: {}", language.code(), issues.join(" | "));
                feedback = issues;
            }
        }
    }
    anyhow::bail!("Claude's clues were rejected {MAX_ATTEMPTS} times; last issues: {}", feedback.join(" | "))
}

#[derive(Debug, Deserialize)]
struct GeneratedFacts {
    countries: Vec<CountryFacts>,
}

#[derive(Debug, Deserialize)]
struct CountryFacts {
    code: String,
    facts: Vec<String>,
}

fn facts_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false, "required": ["countries"],
        "properties": { "countries": { "type": "array", "items": {
            "type": "object", "additionalProperties": false, "required": ["code", "facts"],
            "properties": { "code": { "type": "string" }, "facts": { "type": "array", "items": { "type": "string" } } }
        } } }
    })
}

/// Countries that can be found from `from_day` on but have too few unseen facts in this language.
fn countries_needing_facts(connection: &Connection, language: Language, from_day: &str) -> anyhow::Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT a.country FROM (SELECT DISTINCT country FROM atlas_answers WHERE lang = ?1 AND day >= ?2) a
         LEFT JOIN atlas_facts f ON f.lang = ?1 AND f.country = a.country AND f.shown_on IS NULL
         GROUP BY a.country HAVING COUNT(f.id) < ?3 ORDER BY a.country",
    )?;
    let codes = statement.query_map(params![language.code(), from_day, MIN_UNSEEN_FACTS], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(codes)
}

fn known_facts(connection: &Connection, language: Language, country: &str) -> anyhow::Result<Vec<String>> {
    let mut statement = connection.prepare("SELECT fact FROM atlas_facts WHERE lang = ?1 AND country = ?2")?;
    let facts = statement.query_map(params![language.code(), country], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(facts)
}

fn facts_prompt(connection: &Connection, language: Language, batch: &[String]) -> anyhow::Result<String> {
    let mut prompt = format!(
        "Write fun facts for Atlas Hunt, a geography game. When a player finds a country, they see one fact about it.\n\
         Language: {}.\n\
         For each country below, write {NEW_FACTS_PER_COUNTRY} new facts. Each fact is one or two short sentences (under {MAX_FACT_CHARS} \
         characters), surprising or delightful, true, family-friendly and timeless: no current events, no politicians, no figures that \
         change every year. Mix nature, history, food, culture, records and oddities. Never repeat or rephrase a fact listed as known.\n\n",
        language_name(language)
    );
    for code in batch {
        prompt.push_str(&format!("- {}\n", country_line(code)));
        for fact in known_facts(connection, language, code)? {
            prompt.push_str(&format!("    known: {fact}\n"));
        }
    }
    Ok(prompt)
}

/// Makes sure every country that can be found in the coming days has unseen fun facts. Returns how many were added.
pub async fn top_up_facts(claude: &Claude, connection: &mut Connection, language: Language, from_day: NaiveDate) -> anyhow::Result<usize> {
    let needing = countries_needing_facts(connection, language, &format_day(from_day))?;
    let mut added = 0;
    for batch in needing.chunks(FACT_BATCH) {
        let prompt = facts_prompt(connection, language, batch)?;
        let generated: GeneratedFacts = claude.structured(&prompt, &facts_schema()).await.context("asking Claude for fun facts")?;
        let wanted: HashSet<&str> = batch.iter().map(String::as_str).collect();
        let transaction = connection.transaction()?;
        for country in generated.countries {
            let code = country.code.trim().to_uppercase();
            if !wanted.contains(code.as_str()) {
                continue;
            }
            for fact in country.facts.iter().map(|fact| fact.trim()).filter(|fact| (MIN_FACT_CHARS..=MAX_FACT_CHARS).contains(&fact.chars().count())) {
                added += transaction.execute(
                    "INSERT INTO atlas_facts (lang, country, fact) VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
                    params![language.code(), code, fact],
                )?;
            }
        }
        transaction.commit()?;
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;

    fn data_plan(kind: &str, countries: &[&'static str]) -> Plan {
        Plan::Data(DataClue { kind: kind.into(), meaning: "test".into(), countries: countries.to_vec() })
    }

    fn rated(codes: &[(&str, u8)]) -> Vec<GeneratedCountry> {
        codes.iter().map(|(code, tier)| GeneratedCountry { code: (*code).into(), tier: *tier }).collect()
    }

    #[test]
    fn data_clues_keep_their_answers_and_themes_are_checked_against_the_region() {
        let plans = vec![data_plan("borders:LUX", &["BEL", "FRA", "DEU"]), Plan::Theme(THEMES[0])];
        let generated = Generated {
            clues: vec![
                GeneratedClue { number: 1, clue: "Borders Luxembourg".into(), countries: rated(&[("BEL", 2), ("FRA", 1), ("DEU", 1), ("ESP", 5)]) },
                GeneratedClue { number: 2, clue: "Its flag is red and white only".into(), countries: rated(&[("AUT", 2), ("POL", 1), ("DNK", 3), ("BRA", 4)]) },
            ],
        };
        let issues = validate(Language::En, AtlasRegion::Europe, &plans, generated, &[]).unwrap_err();
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert!(issues[0].contains("BRA"), "{issues:?}");
        assert!(issues[1].contains("needs between"), "{issues:?}");
    }

    #[test]
    fn a_valid_puzzle_passes_and_unrated_data_countries_get_the_middle_tier() {
        let plans = vec![data_plan("landlocked", &["AUT", "CHE", "HUN", "SVK", "CZE"])];
        let generated = Generated {
            clues: vec![GeneratedClue { number: 1, clue: "Has no coastline".into(), countries: rated(&[("AUT", 1), ("CHE", 1), ("HUN", 2), ("SVK", 4)]) }],
        };
        // Data clues may reuse wording (their cooldown keeps them apart), so a recent look-alike is fine.
        let puzzle = validate(Language::En, AtlasRegion::Europe, &plans, generated, &["Has no coastline at all".into()]).unwrap();
        assert!(puzzle[0].answers.contains(&("CZE".to_owned(), DEFAULT_TIER)));
    }

    #[test]
    fn region_changes_drop_unplayed_future_days_only() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&mut connection).unwrap();
        for day in ["2026-11-08", "2026-11-09", "2026-11-10"] {
            connection.execute("INSERT INTO atlas_days VALUES (?1, 'en', 'world')", params![day]).unwrap();
            connection.execute("INSERT INTO atlas_clues VALUES (?1, 'en', 0, 'landlocked', 'No coast')", params![day]).unwrap();
            connection.execute("INSERT INTO atlas_answers VALUES (?1, 'en', 0, 'AUT', 1)", params![day]).unwrap();
        }
        connection.execute("INSERT INTO atlas_plays (day, lang, position, outcome) VALUES ('2026-11-10', 'en', 0, 'playing')", []).unwrap();
        let today = crate::day::parse_day("2026-11-08").unwrap();
        assert_eq!(drop_other_regions(&connection, today, Language::En, AtlasRegion::Asia).unwrap(), 1);
        let left: i64 = connection.query_row("SELECT COUNT(*) FROM atlas_answers", [], |row| row.get(0)).unwrap();
        assert_eq!(left, 2);
    }
}
