import { api } from './api.js';
import { LANGS, LOCALES, translator } from './i18n.js';
import { showDialog } from './dialog.js';
import { formatDuration, startCountdown } from './countdown.js';

const FLIP_DELAY_MS = 450;
const FLIP_STAGGER_MS = 260;
// While a game has no puzzle yet, check this often whether it has arrived.
const WAITING_POLL_MS = 60_000;
// Games whose puzzles Claude writes; Claude's usage limit can hold them back.
const CLAUDE_GAMES = ['rarefind', 'atlas'];

const STRINGS = {
  en: {
    tagline: 'Four daily games, one place',
    language: 'Language',
    played: { one: '{done} of {total} played', other: '{done} of {total} played' },
    newIn: 'New puzzles in',
    claudeLimit: 'Claude’s usage limit is reached: the missing puzzles are made after {time}.',
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
    'atlas.desc': 'Five geography clues. Pick a country; rarer picks rise higher and launch bigger moons.',
    'atlas.rules': 'Each clue fits several countries. Spin the planet and pick one, or type its name. Common picks score a little, rare ones a lot. Three misses and the clue is lost. Every find comes with a fun fact.',
    'atlas.new': '{total} clues waiting', 'atlas.progress': '{done} of {total} clues',
    'atlas.solved': 'All {total} clues done', 'atlas.waiting': 'Today’s planet is still being mapped',
    'snippet.desc': 'Name the song from a tiny clip. Every miss unlocks a longer one.',
    'snippet.rules': 'Press play and name the song. Skip or guess wrong and the next clip is longer. You have six tries. Pick a mode and genre in the settings.',
    'snippet.new': '{total} clips waiting', 'snippet.progress': '{done} of {total} clips used',
    'snippet.solved': 'Named on clip {done} of {total}', 'snippet.failed': 'Not named this time',
    'snippet.waiting': 'Today’s tape is still being recorded',
  },
  fr: {
    tagline: 'Quatre jeux quotidiens, un seul endroit',
    language: 'Langue',
    played: { one: '{done} sur {total} joué', other: '{done} sur {total} joués' },
    newIn: 'Nouveaux défis dans',
    claudeLimit: 'La limite d’utilisation de Claude est atteinte : les défis manquants seront créés après {time}.',
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
    'atlas.desc': 'Cinq indices de géo. Choisis un pays ; les choix rares montent plus haut et lancent de plus grosses lunes.',
    'atlas.rules': 'Chaque indice convient à plusieurs pays. Fais tourner la planète et choisis-en un, ou tape son nom. Un choix courant rapporte peu, un choix rare beaucoup. Trois ratés et l’indice est perdu. Chaque trouvaille vient avec une anecdote.',
    'atlas.new': '{total} indices à jouer', 'atlas.progress': '{done} indices sur {total}',
    'atlas.solved': 'Les {total} indices sont joués', 'atlas.waiting': 'La planète du jour est encore en cartographie',
    'snippet.desc': 'Trouve la chanson à partir d’un mini extrait. Chaque raté débloque un extrait plus long.',
    'snippet.rules': 'Appuie sur lecture et trouve la chanson. Passe ou trompe-toi et l’extrait suivant est plus long. Tu as six essais. Choisis un mode et un genre dans les réglages.',
    'snippet.new': '{total} extraits à écouter', 'snippet.progress': '{done} extraits sur {total}',
    'snippet.solved': 'Trouvée à l’extrait {done} sur {total}', 'snippet.failed': 'Pas trouvée cette fois',
    'snippet.waiting': 'La cassette du jour est encore en enregistrement',
  },
  nl: {
    tagline: 'Vier dagelijkse spellen, één plek',
    language: 'Taal',
    played: { one: '{done} van {total} gespeeld', other: '{done} van {total} gespeeld' },
    newIn: 'Nieuwe puzzels over',
    claudeLimit: 'De gebruikslimiet van Claude is bereikt: de ontbrekende puzzels worden gemaakt na {time}.',
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
    'atlas.desc': 'Vijf aardrijkskundige hints. Kies een land; zeldzame keuzes rijzen hoger en lanceren grotere manen.',
    'atlas.rules': 'Elke hint past bij meerdere landen. Draai de planeet en kies er een, of typ de naam. Een gewone keuze levert weinig op, een zeldzame veel. Drie missers en de hint is verloren. Bij elke vondst hoort een leuk weetje.',
    'atlas.new': '{total} hints klaar', 'atlas.progress': '{done} van {total} hints',
    'atlas.solved': 'Alle {total} hints gespeeld', 'atlas.waiting': 'De planeet van vandaag wordt nog in kaart gebracht',
    'snippet.desc': 'Raad het nummer uit een piepklein fragment. Elke misser ontgrendelt een langer fragment.',
    'snippet.rules': 'Druk op play en raad het nummer. Sla over of raad fout en het volgende fragment is langer. Je hebt zes pogingen. Kies een modus en genre in de instellingen.',
    'snippet.new': '{total} fragmenten klaar', 'snippet.progress': '{done} van {total} fragmenten gebruikt',
    'snippet.solved': 'Geraden bij fragment {done} van {total}', 'snippet.failed': 'Deze keer niet geraden',
    'snippet.waiting': 'De cassette van vandaag wordt nog opgenomen',
  },
};

const ICONS = {
  letterlock: '<rect x="3" y="4" width="5" height="5" rx="1"/><rect x="9.5" y="4" width="5" height="5" rx="1"/><rect x="16" y="4" width="5" height="5" rx="1"/><rect x="3" y="11" width="5" height="5" rx="1"/><rect x="9.5" y="11" width="5" height="5" rx="1" fill="currentColor"/><rect x="16" y="11" width="5" height="5" rx="1"/><path d="M3 20h18"/>',
  rarefind: '<path d="M4 21h16M7 21V9h10v12M9 9V5h6v4M10 13h1M13 13h1M10 17h1M13 17h1M15 5h4M19 5v3"/>',
  atlas: '<circle cx="11" cy="13" r="6.5"/><path d="M4.5 13h13M11 6.5c2 1.8 2 11.2 0 13M11 6.5c-2 1.8-2 11.2 0 13"/><circle cx="19.5" cy="4.5" r="1.8"/><path d="M16.5 3.2c2.6-1.6 5.2-.4 4.3 2.3"/>',
  snippet: '<rect x="2.5" y="5" width="19" height="14" rx="2.5"/><circle cx="8.5" cy="11" r="2"/><circle cx="15.5" cy="11" r="2"/><path d="M10.5 11h3M7 19l1.5-3.5h7L17 19"/>',
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

// Atlas Hunt's result picture: one moon per clue, bigger and brighter for rarer finds (0 = lost, null = not played).
const moonRow = ({ tiers }) => `<span class="moon-row" aria-hidden="true">${tiers.map(tier =>
  `<span class="${tier === null ? '' : tier === 0 ? 'm-lost' : `m-${tier}`}"></span>`).join('')}</span>`;

// Snippet's result picture: one strip per clip, marked skipped, wrong or right.
const clipRow = ({ marks, max }) => `<span class="clip-row" aria-hidden="true">${Array.from({ length: max }, (_, index) =>
  `<span class="${marks[index] ? `c-${marks[index]}` : ''}"></span>`).join('')}</span>`;

// Every game, with its page and result picture.
const GAMES = {
  letterlock: { name: 'Letterlock', href: 'games/letterlock/', picture: letterGrid },
  rarefind: { name: 'Rarefind', href: 'games/rarefind/', picture: towerRow },
  atlas: { name: 'Atlas Hunt', href: 'games/atlas/', picture: moonRow },
  snippet: { name: 'Snippet', href: 'games/snippet/', picture: clipRow },
};

const home = document.getElementById('home');
let t = translator(STRINGS, 'en');
let stopCountdown = () => {};
let flipTimers = [];
let waitingTimer = 0;

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

// Says when Claude's usage limit lifts, if it is what keeps a game waiting.
function claudeLimitHtml(summary, games, lang) {
  const held = games.some(game => game.status === 'waiting' && CLAUDE_GAMES.includes(game.id));
  if (!summary.claudePausedUntil || !held) return '';
  const time = new Intl.DateTimeFormat(LOCALES[lang], { hour: '2-digit', minute: '2-digit' }).format(new Date(summary.claudePausedUntil));
  return `<p class="muted">${t('claudeLimit', { time })}</p>`;
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
      ${claudeLimitHtml(summary, games, lang)}
    </section>
    <ul class="tile-grid">${games.map(tile).join('')}</ul>`;
  home.setAttribute('aria-busy', 'false');
  flipSolvedTiles();
  stopCountdown();
  const countdown = home.querySelector('[data-countdown]');
  stopCountdown = startCountdown(summary.secondsToReset, left => { countdown.textContent = formatDuration(left); }, load);
  clearTimeout(waitingTimer);
  if (games.some(game => game.status === 'waiting')) waitingTimer = setTimeout(load, WAITING_POLL_MS);
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
