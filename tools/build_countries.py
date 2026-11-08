"""Build Atlas Hunt's country data: names in English, French and Dutch, facts used for clues, and the world map.

Usage:  python3 tools/build_countries.py
Downloads two pinned sources (cached in tools/.cache/) and writes:
  web/games/atlas/data/countries.json   one entry per playable country (read by the browser and compiled into the server)
  web/games/atlas/data/world.json       country shapes (TopoJSON, Natural Earth 1:50m)
Running it again rebuilds the same files from the cache.

Sources and licences:
  mledoze/countries  (names, translations, borders, languages, currencies, area)  ODbL 1.0
  world-atlas 2.0.2  (Natural Earth 1:50m shapes)                                 ISC; Natural Earth is public domain
"""
import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "tools" / ".cache"
OUT = ROOT / "web" / "games" / "atlas" / "data"
COUNTRIES_URL = "https://cdn.jsdelivr.net/gh/mledoze/countries@fa4f62182d19ff4f8b325cde6387c9cd9d80955a/countries.json"
WORLD_URL = "https://cdn.jsdelivr.net/npm/world-atlas@2.0.2/countries-50m.json"
# UN members plus the two UN observer states, Kosovo and Taiwan.
EXTRA_COUNTRIES = {"VAT", "PSE", "UNK", "TWN"}
# Map shapes without a numeric code in world-atlas, matched by their map name instead.
MAP_NAMES = {"UNK": "Kosovo"}
MIN_ALIAS_LENGTH = 4


def cached(url: str, name: str) -> bytes:
    path = CACHE / name
    if not path.exists():
        CACHE.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(url, timeout=60) as response:
            path.write_bytes(response.read())
    return path.read_bytes()


def aliases(entry: dict, names: dict) -> list[str]:
    """Other names a player might type: official names and common alternative spellings (not 2-letter codes)."""
    candidates = [entry["name"]["official"]]
    candidates += [entry["translations"][key]["official"] for key in ("fra", "nld") if key in entry["translations"]]
    candidates += entry.get("altSpellings", [])
    seen = {name.casefold() for name in names.values()}
    result = []
    for candidate in candidates:
        if len(candidate) >= MIN_ALIAS_LENGTH and candidate.casefold() not in seen:
            seen.add(candidate.casefold())
            result.append(candidate)
    return result


def build(countries: list, world: dict) -> tuple[list, dict, list]:
    playable = [entry for entry in countries if entry.get("unMember") or entry["cca3"] in EXTRA_COUNTRIES]
    codes = {entry["cca3"] for entry in playable}
    geometries = world["objects"]["countries"]["geometries"]
    map_ids = {geometry["id"] for geometry in geometries if geometry.get("id")}
    map_names = {geometry["properties"]["name"] for geometry in geometries}
    result = []
    without_shape = []
    for entry in sorted(playable, key=lambda item: item["cca3"]):
        code = entry["cca3"]
        names = {
            "en": entry["name"]["common"],
            "fr": entry["translations"].get("fra", {}).get("common", entry["name"]["common"]),
            "nl": entry["translations"].get("nld", {}).get("common", entry["name"]["common"]),
        }
        if entry.get("ccn3") in map_ids:
            shape = entry["ccn3"]
        elif MAP_NAMES.get(code) in map_names:
            shape = MAP_NAMES[code]
        else:
            shape = None
            without_shape.append(names["en"])
        result.append({
            "code": code,
            "shape": shape,
            "names": names,
            "aliases": aliases(entry, names),
            "region": entry["region"],
            "subregion": entry.get("subregion", ""),
            "borders": sorted(border for border in entry.get("borders", []) if border in codes),
            "landlocked": bool(entry.get("landlocked")),
            "area": entry.get("area", 0),
            "languages": sorted(entry.get("languages", {}).values()),
            "currencies": sorted(entry.get("currencies", {}).keys()),
            "capital": (entry.get("capital") or [""])[0],
            "latlng": entry.get("latlng", [0, 0]),
        })
    # Keep only the country shapes; the separate "land" outline is not used.
    slim = {**world, "objects": {"countries": world["objects"]["countries"]}}
    return result, slim, without_shape


def main() -> int:
    try:
        countries = json.loads(cached(COUNTRIES_URL, "mledoze-countries.json"))
        world = json.loads(cached(WORLD_URL, "world-atlas-countries-50m.json"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"error: could not download or read the sources: {error}", file=sys.stderr)
        return 1
    result, slim, without_shape = build(countries, world)
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "countries.json").write_text(json.dumps(result, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    (OUT / "world.json").write_text(json.dumps(slim, separators=(",", ":")), encoding="utf-8")
    print(f"Countries: {len(result)} written to {OUT / 'countries.json'}")
    print(f"Map: {len(slim['objects']['countries']['geometries'])} shapes written to {OUT / 'world.json'}")
    if without_shape:
        print(f"Shown as a pin (no shape at 1:50m): {', '.join(without_shape)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
