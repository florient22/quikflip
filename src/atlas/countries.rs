//! The playable countries (built by tools/build_countries.py). The same file feeds the browser, so names and codes
//! always agree between the map and the server.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::settings::AtlasRegion;

const COUNTRIES_JSON: &str = include_str!("../../web/games/atlas/data/countries.json");
/// Land borders through overseas territories, which the data lists under the territory instead of the country.
/// Territories count for the country they belong to: French Guiana (Brazil, Suriname), Saint Martin (France and the
/// Netherlands), Gibraltar (Spain), and Akrotiri and Dhekelia (Cyprus).
const OVERSEAS_BORDERS: &[(&str, &str)] = &[("FRA", "BRA"), ("FRA", "SUR"), ("FRA", "NLD"), ("GBR", "ESP"), ("GBR", "CYP")];

/// The server only needs the English name (for Claude); the browser shows all three from the same file.
#[derive(Debug, Deserialize)]
pub struct Names {
    pub en: String,
}

#[derive(Debug, Deserialize)]
pub struct Country {
    /// ISO 3166-1 alpha-3, e.g. "BEL".
    pub code: String,
    pub names: Names,
    pub region: String,
    /// Land neighbours, including those through overseas territories.
    pub borders: Vec<String>,
    pub landlocked: bool,
    /// Official languages, in English.
    pub languages: Vec<String>,
    /// ISO 4217 codes.
    pub currencies: Vec<String>,
}

impl Country {
    pub fn in_region(&self, region: AtlasRegion) -> bool {
        region.dataset_name().is_none_or(|name| self.region == name)
    }
}

static COUNTRIES: LazyLock<Vec<Country>> = LazyLock::new(|| {
    let mut countries: Vec<Country> =
        serde_json::from_str(COUNTRIES_JSON).expect("web/games/atlas/data/countries.json is compiled in and checked by a test");
    for (first, second) in OVERSEAS_BORDERS {
        for (country, neighbour) in [(first, second), (second, first)] {
            if let Some(entry) = countries.iter_mut().find(|entry| entry.code == **country)
                && !entry.borders.iter().any(|code| code == neighbour)
            {
                entry.borders.push((*neighbour).to_owned());
            }
        }
    }
    countries
});
static BY_CODE: LazyLock<HashMap<&'static str, &'static Country>> =
    LazyLock::new(|| COUNTRIES.iter().map(|country| (country.code.as_str(), country)).collect());

pub fn all() -> &'static [Country] {
    &COUNTRIES
}

pub fn by_code(code: &str) -> Option<&'static Country> {
    BY_CODE.get(code).copied()
}

pub fn in_region(region: AtlasRegion) -> impl Iterator<Item = &'static Country> {
    COUNTRIES.iter().filter(move |country| country.in_region(region))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compiled_in_country_list_reads() {
        assert!(all().len() >= 190);
        let belgium = by_code("BEL").unwrap();
        assert_eq!(belgium.names.en, "Belgium");
        assert!(belgium.borders.contains(&"FRA".to_owned()));
        assert!(by_code("BRA").unwrap().borders.contains(&"FRA".to_owned()), "French Guiana makes France a neighbour");
        assert_eq!(in_region(AtlasRegion::Oceania).count(), 14);
        // Every border points at a country in the list.
        assert!(all().iter().flat_map(|country| &country.borders).all(|code| by_code(code).is_some()));
    }
}
