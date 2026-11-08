// Translation helper shared by every page. Each page brings its own strings; English is the fallback.

export const LANGS = ['en', 'fr', 'nl'];
export const LOCALES = { en: 'en-GB', fr: 'fr-BE', nl: 'nl-BE' };

const fill = (text, vars) => text.replace(/\{(\w+)\}/g, (_, name) => vars[name] ?? '');

/**
 * Returns t(key, vars). A string may also be an object of plural forms ({ one, other }),
 * chosen with Intl.PluralRules from vars.count.
 */
export function translator(strings, lang) {
  const plural = new Intl.PluralRules(LOCALES[lang] ?? LOCALES.en);
  return (key, vars = {}) => {
    const entry = strings[lang]?.[key] ?? strings.en[key];
    if (entry === undefined) return key;
    if (typeof entry === 'string') return fill(entry, vars);
    const form = entry[plural.select(vars.count ?? 0)] ?? entry.other;
    return fill(form, vars);
  };
}
