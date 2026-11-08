import { api } from './api.js';
import { LANGS, LOCALES, translator } from './i18n.js';
import { showDialog } from './dialog.js';
import { formatDuration, startCountdown } from './countdown.js';

const FLIP_DELAY_MS = 450;
const FLIP_STAGGER_MS = 260;

const STRINGS = {
  en: {
    tagline: 'Four daily games, one place',
    language: 'Language',
    played: { one: '{done} of {total} played', other: '{done} of {total} played' },
    newIn: 'New puzzles in',
    howTo: 'How to play',
    close: 'Close',
    streak: { one: '1-day streak', other: '{count}-day streak' },
    noStreak: 'No streak yet',
    loadFailed: 'Couldn’t reach QuikFlip. Check that the server is running, then try again.',
    saveFailed: 'Couldn’t save that setting. Try again in a moment.',
    retry: 'Try again',
    problem: 'Something went wrong',
    'status.new': 'Not played', 'status.progress': 'In progress', 'status.solved': 'Solved', 'status.failed': 'Out of tries',
    'action.new': 'Play', 'action.progress': 'Continue', 'action.solved': 'See result', 'action.failed': 'See result',
    'letterlock.desc': 'Crack the five-letter word in six tries.',
    'letterlock.rules': 'Type a five-letter word. Green means right letter, right spot. Yellow means the letter is in the word but somewhere else. Grey means it is not in the word.',
    'letterlock.new': '{total} tries waiting', 'letterlock.progress': '{done} of {total} tries used',
    'letterlock.solved': 'Solved in {done} of {total}', 'letterlock.failed': 'All {total} tries used',
    'status.waiting': 'Not ready yet', 'action.waiting': 'Peek',
    'rarefind.desc': 'Seven prompts. Every answer adds a floor; rarer answers build higher.',
    'rarefind.rules': 'Each prompt accepts lots of answers. Type one before the crane gets tired. Obvious answers build a plain floor; rare ones build tall, fancy floors.',
    'rarefind.new': '{total} floors to build', 'rarefind.progress': '{done} of {total} floors',
    'rarefind.solved': 'All {total} floors done', 'rarefind.waiting': 'Today’s floors are still being drawn',
  },
  fr: {
    tagline: 'Quatre jeux quotidiens, un seul endroit',
    language: 'Langue',
    played: { one: '{done} sur {total} joué', other: '{done} sur {total} joués' },
    newIn: 'Nouveaux défis dans',
    howTo: 'Comment jouer',
    close: 'Fermer',
    streak: { one: '1 jour d’affilée', other: '{count} jours d’affilée' },
    noStreak: 'Pas encore de série',
    loadFailed: 'Impossible de joindre QuikFlip. Vérifie que le serveur tourne, puis réessaie.',
    saveFailed: 'Impossible d’enregistrer ce réglage. Réessaie dans un instant.',
    retry: 'Réessayer',
    problem: 'Un problème est survenu',
    'status.new': 'Pas joué', 'status.progress': 'En cours', 'status.solved': 'Terminé', 'status.failed': 'Plus d’essais',
    'action.new': 'Jouer', 'action.progress': 'Continuer', 'action.solved': 'Voir le résultat', 'action.failed': 'Voir le résultat',
    'letterlock.desc': 'Trouve le mot de cinq lettres en six essais.',
    'letterlock.rules': 'Propose un mot de cinq lettres. Vert : bonne lettre, bonne place. Jaune : la lettre est dans le mot, mais ailleurs. Gris : elle n’y est pas.',
    'letterlock.new': '{total} essais disponibles', 'letterlock.progress': '{done} essais sur {total}',
    'letterlock.solved': 'Trouvé en {done} sur {total}', 'letterlock.failed': 'Les {total} essais sont utilisés',
    'status.waiting': 'Pas encore prêt', 'action.waiting': 'Jeter un œil',
    'rarefind.desc': 'Sept questions. Chaque réponse ajoute un étage ; les plus rares montent plus haut.',
    'rarefind.rules': 'Chaque question accepte beaucoup de réponses. Tapes-en une avant que la grue fatigue. Une réponse évidente bâtit un étage simple ; une rare bâtit un étage haut et chic.',
    'rarefind.new': '{total} étages à bâtir', 'rarefind.progress': '{done} étages sur {total}',
    'rarefind.solved': 'Les {total} étages sont bâtis', 'rarefind.waiting': 'Les étages du jour sont encore en dessin',
  },
  nl: {
    tagline: 'Vier dagelijkse spellen, één plek',
    language: 'Taal',
    played: { one: '{done} van {total} gespeeld', other: '{done} van {total} gespeeld' },
    newIn: 'Nieuwe puzzels over',
    howTo: 'Spelregels',
    close: 'Sluiten',
    streak: { one: '1 dag op rij', other: '{count} dagen op rij' },
    noStreak: 'Nog geen reeks',
    loadFailed: 'QuikFlip is niet bereikbaar. Controleer of de server draait en probeer opnieuw.',
    saveFailed: 'Die instelling kon niet worden opgeslagen. Probeer het zo meteen opnieuw.',
    retry: 'Opnieuw proberen',
    problem: 'Er ging iets mis',
    'status.new': 'Niet gespeeld', 'status.progress': 'Bezig', 'status.solved': 'Klaar', 'status.failed': 'Geen pogingen meer',
    'action.new': 'Spelen', 'action.progress': 'Verder', 'action.solved': 'Resultaat', 'action.failed': 'Resultaat',
    'letterlock.desc': 'Kraak het woord van vijf letters in zes pogingen.',
    'letterlock.rules': 'Typ een woord van vijf letters. Groen: juiste letter op de juiste plek. Geel: de letter zit in het woord, maar ergens anders. Grijs: de letter zit er niet in.',
    'letterlock.new': '{total} pogingen klaar', 'letterlock.progress': '{done} van {total} pogingen gebruikt',
    'letterlock.solved': 'Geraden in {done} van {total}', 'letterlock.failed': 'Alle {total} pogingen gebruikt',
    'status.waiting': 'Nog niet klaar', 'action.waiting': 'Kijken',
    'rarefind.desc': 'Zeven vragen. Elk antwoord voegt een verdieping toe; zeldzame antwoorden bouwen hoger.',
    'rarefind.rules': 'Elke vraag heeft veel goede antwoorden. Typ er een voor de kraan moe wordt. Een voor de hand liggend antwoord bouwt een gewone verdieping; een zeldzaam antwoord een hoge, chique.',
    'rarefind.new': '{total} verdiepingen te bouwen', 'rarefind.progress': '{done} van {total} verdiepingen',
    'rarefind.solved': 'Alle {total} verdiepingen klaar', 'rarefind.waiting': 'De verdiepingen van vandaag worden nog getekend',
  },
};

const ICONS = {
  letterlock: '<rect x="3" y="4" width="5" height="5" rx="1"/><rect x="9.5" y="4" width="5" height="5" rx="1"/><rect x="16" y="4" width="5" height="5" rx="1"/><rect x="3" y="11" width="5" height="5" rx="1"/><rect x="9.5" y="11" width="5" height="5" rx="1" fill="currentColor"/><rect x="16" y="11" width="5" height="5" rx="1"/><path d="M3 20h18"/>',
  rarefind: '<path d="M4 21h16M7 21V9h10v12M9 9V5h6v4M10 13h1M13 13h1M10 17h1M13 17h1M15 5h4M19 5v3"/>',
  flame: '<path d="M12 21c-3.9 0-6.5-2.6-6.5-6.1 0-3.3 2.4-5.4 3.8-8.4.4 2 1.6 3.3 2.9 3.6.2-2.6 1.3-5 3.3-7.1.4 3.5 3.5 5.9 3.5 10.2 0 4.4-3 7.8-7 7.8z"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 7.6v.2"/>',
  play: '<path d="M8 5.5v13l10-6.5z"/>',
};
const icon = (name, className = 'icon') => `<svg class="${className}" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name]}</svg>`;

const MARK_CLASS = { g: 'm-g', y: 'm-y', x: 'm-x' };
const letterGrid = ({ marks, max }) => {
  const rows = Array.from({ length: max }, (_, index) => marks[index] ?? '     ');
  return `<span class="letter-grid" aria-hidden="true">${rows.map(row => [...row].map(mark => `<span class="${MARK_CLASS[mark] ?? ''}"></span>`).join('')).join('')}</span>`;
};

// Rarefind's result picture: one block per prompt, coloured by the floor it built (0 = no floor, null = not played).
const towerRow = ({ tiers }) => `<span class="tower-row" aria-hidden="true">${tiers.map(tier =>
  `<span class="${tier === null ? '' : tier === 0 ? 't-none' : `t-${tier}`}"></span>`).join('')}</span>`;

// Games that exist so far. Each new game registers here with its page and result picture.
const GAMES = {
  letterlock: { name: 'Letterlock', href: 'games/letterlock/', picture: letterGrid },
  rarefind: { name: 'Rarefind', href: 'games/rarefind/', picture: towerRow },
};

const home = document.getElementById('home');
let t = translator(STRINGS, 'en');
let stopCountdown = () => {};
let flipTimers = [];

const finished = status => status === 'solved' || status === 'failed';

function formatDate(isoDate, lang) {
  const [year, month, day] = isoDate.split('-').map(Number);
  return new Intl.DateTimeFormat(LOCALES[lang], { weekday: 'long', day: 'numeric', month: 'long' }).format(new Date(year, month - 1, day));
}

const logo = `
  <svg class="logo" viewBox="0 0 32 32" aria-hidden="true">
    <rect class="logo-back" x="10" y="3" width="17" height="21" rx="4" transform="rotate(12 18.5 13.5)"/>
    <rect class="logo-front" x="5" y="8" width="17" height="21" rx="4"/>
    <path class="logo-check" d="M9.5 18.5l3 3 5.5-6.5"/>
  </svg>`;

const header = lang => `
  <header class="home-head">
    <div class="brand">${logo}<div><h1 class="wordmark">QuikFlip</h1><p class="tagline">${t('tagline')}</p></div></div>
    <div class="lang" role="group" aria-label="${t('language')}">
      ${LANGS.map(code => `<button type="button" data-lang="${code}" aria-pressed="${code === lang}">${code.toUpperCase()}</button>`).join('')}
    </div>
  </header>`;

function tile(game) {
  const meta = GAMES[game.id];
  const result = t(`${game.id}.${game.status}`, { done: game.used, total: game.max });
  const streak = game.streak ? t('streak', { count: game.streak }) : t('noStreak');
  return `
    <li class="tile" data-flip="${game.status === 'solved'}">
      <a class="tile-card" href="${meta.href}" aria-label="${meta.name}: ${t(`status.${game.status}`)}, ${result}">
        <span class="face face-front" aria-hidden="true">
          ${icon(game.id, 'icon game-icon')}
          <span class="game-name">${meta.name}</span>
          <span class="game-desc">${t(`${game.id}.desc`)}</span>
          ${meta.picture(game)}
          <span class="tile-foot"><span class="pill pill-${game.status}">${t(`status.${game.status}`)}</span><span class="tile-action">${icon('play')}${t(`action.${game.status}`)}</span></span>
        </span>
        <span class="face face-back" aria-hidden="true">
          ${icon(game.id, 'icon game-icon')}
          <span class="game-name">${meta.name}</span>
          ${meta.picture(game)}
          <span class="game-result">${result}</span>
          <span class="tile-foot"><span class="streak">${icon('flame')}${streak}</span></span>
        </span>
      </a>
      <button type="button" class="icon-btn" data-howto="${game.id}" aria-label="${t('howTo')}: ${meta.name}">${icon('info')}</button>
    </li>`;
}

function flipSolvedTiles() {
  const tiles = [...home.querySelectorAll('.tile[data-flip="true"]')];
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
    tiles.forEach(element => element.classList.add('is-flipped'));
    return;
  }
  flipTimers = tiles.map((element, index) => setTimeout(() => element.classList.add('is-flipped'), FLIP_DELAY_MS + index * FLIP_STAGGER_MS));
}

function render(summary) {
  const lang = summary.language;
  t = translator(STRINGS, lang);
  document.documentElement.lang = lang;
  const games = summary.games.filter(game => GAMES[game.id]);
  const done = games.filter(game => finished(game.status)).length;
  home.innerHTML = `
    ${header(lang)}
    <section class="summary">
      <p class="headline">${formatDate(summary.date, lang)}</p>
      <div class="day-bar" aria-hidden="true">${games.map(game => `<span data-status="${game.status}"></span>`).join('')}</div>
      <p class="muted">${t('played', { done, total: games.length, count: done })} &middot; ${t('newIn')} <span class="countdown" data-countdown></span></p>
    </section>
    <ul class="tile-grid">${games.map(tile).join('')}</ul>`;
  home.setAttribute('aria-busy', 'false');
  flipSolvedTiles();
  stopCountdown();
  const countdown = home.querySelector('[data-countdown]');
  stopCountdown = startCountdown(summary.secondsToReset, left => { countdown.textContent = formatDuration(left); }, load);
}

function renderError() {
  home.innerHTML = `<section class="notice" role="alert"><p>${t('loadFailed')}</p><button type="button" data-retry>${t('retry')}</button></section>`;
  home.setAttribute('aria-busy', 'false');
}

async function load() {
  flipTimers.forEach(clearTimeout);
  flipTimers = [];
  try {
    render(await api.home());
  } catch {
    renderError();
  }
}

home.addEventListener('click', async event => {
  const target = event.target.closest('[data-lang], [data-howto], [data-retry]');
  if (!target) return;
  const { lang, howto } = target.dataset;
  if (lang) {
    try {
      await api.saveAppSettings({ language: lang });
      await load();
    } catch {
      showDialog({ title: t('problem'), content: `<p>${t('saveFailed')}</p>`, closeLabel: t('close') });
    }
  } else if (howto) {
    showDialog({ title: `${t('howTo')}: ${GAMES[howto].name}`, content: `<p>${t(`${howto}.rules`)}</p>`, closeLabel: t('close') });
  } else {
    load();
  }
});

load();
