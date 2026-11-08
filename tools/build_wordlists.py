"""Build Letterlock's word lists (answers + allowed guesses) for English, French and Dutch.

Usage:  python3 tools/build_wordlists.py
Downloads are cached in tools/.cache, so running it again is fast and gives the same result.
Writes words/{lang}-answers.txt and words/{lang}-allowed.txt, then prints a summary.
"""
import json
import re
import sys
import unicodedata
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / "tools" / ".cache"
OUTPUT = ROOT / "words"
WORD_LENGTH = 5
FIVE_LETTERS = re.compile(r"^[a-z]{%d}$" % WORD_LENGTH)
FREQUENCY_URL = "https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/{lang}/{lang}_50k.txt"

LANGUAGES = {
    "en": {"dictionary": "https://raw.githubusercontent.com/dolph/dictionary/master/enable1.txt", "format": "lines", "answers": 2500},
    "nl": {"dictionary": "https://raw.githubusercontent.com/OpenTaal/opentaal-wordlist/master/wordlist.txt", "format": "lines", "answers": 1800},
    "fr": {"dictionary": "https://raw.githubusercontent.com/words/an-array-of-french-words/master/index.json", "format": "json", "answers": 1800},
}
LIGATURES = {"œ": "oe", "æ": "ae", "ß": "ss"}


def download(url: str) -> str:
    CACHE.mkdir(parents=True, exist_ok=True)
    target = CACHE / re.sub(r"[^A-Za-z0-9._-]", "_", url.split("://", 1)[1])
    if not target.exists():
        print(f"  downloading {url}")
        with urllib.request.urlopen(url, timeout=60) as response:
            target.write_bytes(response.read())
    return target.read_text(encoding="utf-8")


def fold(word: str) -> str:
    """Lowercase, expand ligatures, strip accents: 'Élève' -> 'eleve'."""
    word = word.strip().lower()
    for ligature, replacement in LIGATURES.items():
        word = word.replace(ligature, replacement)
    return "".join(char for char in unicodedata.normalize("NFD", word) if not unicodedata.combining(char))


def dictionary_words(spec: dict) -> set:
    text = download(spec["dictionary"])
    raw = json.loads(text) if spec["format"] == "json" else text.splitlines()
    words = set()
    for entry in raw:
        entry = entry.strip()
        # Capitalised entries are names or places; skip them.
        if not entry or entry != entry.lower():
            continue
        folded = fold(entry)
        if FIVE_LETTERS.match(folded):
            words.add(folded)
    return words


def frequency_ranked(lang: str) -> list:
    ranked = []
    seen = set()
    for line in download(FREQUENCY_URL.format(lang=lang)).splitlines():
        word = fold(line.split(" ", 1)[0])
        if word not in seen:
            seen.add(word)
            ranked.append(word)
    return ranked


def blocklist(lang: str) -> set:
    path = ROOT / "tools" / f"blocklist-{lang}.txt"
    if not path.exists():
        return set()
    return {fold(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip() and not line.startswith("#")}


def is_english_plural(word: str, dictionary_four: set) -> bool:
    return word.endswith("s") and not word.endswith("ss") and word[:-1] in dictionary_four


def build(lang: str, spec: dict) -> tuple:
    allowed = dictionary_words(spec)
    blocked = blocklist(lang)
    four_letter = set()
    if lang == "en":
        four_letter = {fold(line) for line in download(spec["dictionary"]).splitlines() if len(line.strip()) == WORD_LENGTH - 1}
    answers = []
    for word in frequency_ranked(lang):
        if len(answers) >= spec["answers"]:
            break
        if word in allowed and word not in blocked and not (lang == "en" and is_english_plural(word, four_letter)):
            answers.append(word)
    allowed -= blocked
    allowed.update(answers)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    (OUTPUT / f"{lang}-answers.txt").write_text("\n".join(sorted(answers)) + "\n", encoding="utf-8")
    (OUTPUT / f"{lang}-allowed.txt").write_text("\n".join(sorted(allowed)) + "\n", encoding="utf-8")
    return len(answers), len(allowed), len(blocked)


def main() -> int:
    print("Building Letterlock word lists")
    try:
        results = {lang: build(lang, spec) for lang, spec in LANGUAGES.items()}
    except OSError as error:
        print(f"error: could not download or write a word list: {error}", file=sys.stderr)
        return 1
    for lang, (answers, allowed, blocked) in results.items():
        print(f"  {lang}: {answers} answers, {allowed} allowed guesses, {blocked} blocked words")
    print(f"Wrote {len(results) * 2} files to {OUTPUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
