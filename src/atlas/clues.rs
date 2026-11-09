//! Clues whose answers are computed from the country data, so the list of valid countries is always complete and
//! correct. Claude only words them and rates how rare each country is. Only facts that make good trivia are used
//! (neighbours, languages, currencies, coastlines); regions, sizes and first letters made dull, school-test clues.

use std::collections::BTreeMap;

use super::countries::{self, Country};
use crate::settings::AtlasRegion;

/// A clue needs at least this many valid countries, so there is something rare to find.
pub const MIN_VALID: usize = 4;
const MANY_NEIGHBOURS: usize = 6;
const FEW_NEIGHBOURS: [usize; 3] = [1, 2, 3];
/// Currencies shared by enough countries to make a clue, with the name Claude should use.
const CURRENCIES: &[(&str, &str)] = &[
    ("EUR", "the euro"),
    ("USD", "the US dollar"),
    ("XOF", "the West African CFA franc"),
    ("XAF", "the Central African CFA franc"),
    ("XCD", "the East Caribbean dollar"),
    ("ZAR", "the South African rand"),
    ("AUD", "the Australian dollar"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct DataClue {
    /// Stable key used for the cooldown, e.g. "language:French".
    pub kind: String,
    /// What the clue means, in English, for Claude to word in the player's language.
    pub meaning: String,
    pub countries: Vec<&'static str>,
}

fn clue(kind: String, meaning: String, matching: Vec<&'static Country>) -> Option<DataClue> {
    (matching.len() >= MIN_VALID).then(|| DataClue { kind, meaning, countries: matching.iter().map(|country| country.code.as_str()).collect() })
}

/// Every data clue that works for this region. The generator filters out recently used kinds.
pub fn candidates(region: AtlasRegion) -> Vec<DataClue> {
    let pool: Vec<&'static Country> = countries::in_region(region).collect();
    let select = |keep: &dyn Fn(&Country) -> bool| pool.iter().copied().filter(|country| keep(country)).collect::<Vec<_>>();
    let mut clues = Vec::new();

    for neighbour in countries::all() {
        let name = &neighbour.names.en;
        clues.extend(clue(format!("borders:{}", neighbour.code), format!("Shares a land border with {name}"), select(&|country| country.borders.contains(&neighbour.code))));
    }

    let mut languages: BTreeMap<&str, Vec<&'static Country>> = BTreeMap::new();
    for country in &pool {
        for spoken in &country.languages {
            languages.entry(spoken.as_str()).or_default().push(country);
        }
    }
    for (spoken, matching) in languages {
        clues.extend(clue(format!("language:{spoken}"), format!("{spoken} is an official language there"), matching));
    }

    for (code, name) in CURRENCIES {
        clues.extend(clue(format!("currency:{code}"), format!("Uses {name} as its currency"), select(&|country| country.currencies.iter().any(|currency| currency == code))));
    }

    clues.extend(clue("landlocked".into(), "Is landlocked (no coastline at all)".into(), select(&|country| country.landlocked)));
    clues.extend(clue("no-borders".into(), "Has no land border with any other country (islands and island groups)".into(), select(&|country| country.borders.is_empty())));
    for count in FEW_NEIGHBOURS {
        clues.extend(clue(format!("neighbours:{count}"), format!("Borders exactly {count} other countr{}", if count == 1 { "y" } else { "ies" }), select(&|country| country.borders.len() == count)));
    }
    clues.extend(clue(format!("neighbours:{MANY_NEIGHBOURS}+"), format!("Borders {MANY_NEIGHBOURS} or more countries"), select(&|country| country.borders.len() >= MANY_NEIGHBOURS)));
    clues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(clues: &'a [DataClue], kind: &str) -> &'a DataClue {
        clues.iter().find(|clue| clue.kind == kind).unwrap_or_else(|| panic!("no clue {kind}"))
    }

    #[test]
    fn data_clues_list_every_matching_country() {
        let world = candidates(AtlasRegion::World);
        let germany = find(&world, "borders:DEU");
        assert_eq!(germany.countries.len(), 9);
        assert!(germany.countries.contains(&"BEL"));
        assert!(find(&world, "landlocked").countries.contains(&"CHE"));
        assert!(world.iter().all(|clue| clue.countries.len() >= MIN_VALID));
    }

    #[test]
    fn regions_change_the_answers_and_territories_count() {
        let europe = candidates(AtlasRegion::Europe);
        assert!(find(&europe, "language:French").countries.iter().all(|code| countries::by_code(code).unwrap().region == "Europe"));
        assert!(find(&candidates(AtlasRegion::World), "borders:BRA").countries.contains(&"FRA"));
    }
}
