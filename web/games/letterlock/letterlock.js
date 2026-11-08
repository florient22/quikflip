import { api } from '../../js/api.js';
import { LOCALES, translator } from '../../js/i18n.js';
import { showDialog } from '../../js/dialog.js';
import { formatDuration, startCountdown } from '../../js/countdown.js';

const ENTER = 'Enter';
const BACK = 'Backspace';
const KEY_ROWS = {
  qwerty: [[...'qwertyuiop'], [...'asdfghjkl'], [ENTER, ...'zxcvbnm', BACK]],
  azerty: [[...'azertyuiop'], [...'qsdfghjklm'], [ENTER, ...'wxcvbn', BACK]],
};
const PICK_MS = 500;
const REVEAL_STAGGER_MS = 260;
const REVEAL_SETTLE_MS = 620;
const SHACKLE_LIFT_PER_LETTER = 4;
const CONFETTI_COUNT = 26;
const SFX_MS = 1500;
const LOCK_EPOCH = Date.UTC(2026, 10, 8);   // QuikFlip's first day is lock no. 1
const DAY_MS = 86_400_000;
const MARK_RANK = { x: 1, y: 2, g: 3 };
// Swatches only; the ink values themselves live in the CSS tokens.
const INK_SETS = [
  { id: 'tangerine', swatches: ['#263266', '#ff6c2f', '#0f8a83', '#ffb511'] },
  { id: 'forest', swatches: ['#24443a', '#ff6f6a', '#3a9d6e', '#f0b93a'] },
  { id: 'newsprint', swatches: ['#1c1c1e', '#e5392a', '#e8e6df'] },
];
const MOUTHS = {
  idle: 'M118 186 Q130 192 142 186',
  smug: 'M116 184 Q132 194 146 180',
  nervous: 'M116 190 Q123 184 130 190 Q137 196 144 190',
  panic: 'M120 182 Q130 200 140 182 Z',
  dizzy: 'M116 190 Q130 180 144 190',
};

const STRINGS = {
  en: {
    back: 'Back to QuikFlip', help: 'How to play', settings: 'Settings', close: 'Close', guesses: 'Guesses',
    lockNo: 'Lock No. {n}', tumblers: 'Tumblers', pinsLabel: 'Letters locked in place: {list}', unknown: 'unknown',
    enter: 'Pick', enterLabel: 'Submit guess', delete: 'Delete',
    rules: 'Type a five-letter word and press Pick. Green means right letter, right spot. Yellow means the letter is in the word but somewhere else. Grey means it is not in the word. You have six tries.',
    start: 'Lock {n}. Five letters, six tries. Bet you can’t crack me.',
    resume: { one: 'Back for more? You’ve got 1 try left.', other: 'Back for more? You’ve got {count} tries left.' },
    panic: { one: 'Hey, stop that! 1 letter locked in.', other: 'Hey, stop that! {count} letters locked in.' },
    nervous: { one: 'Hm. 1 letter clicked. Lucky guess.', other: 'Hm. {count} letters clicked. Lucky guess.' },
    smugNear: { one: 'Warm, but wrong slot. 1 letter rattling around.', other: 'Warm, but wrong slots. {count} letters rattling around.' },
    smugNone: 'Ha. Not a single one of those letters.',
    last: 'One try left. I like my chances.',
    short: 'Five letters, please. I’m a professional.',
    notWord: '“{word}”? That’s not a word I know.',
    hardGreen: 'Hard mode: letter {position} must be {letter}.',
    hardPresent: 'Hard mode: you must use {letter}.',
    won: { one: 'Fine, fine. You cracked me in 1 try. Show-off.', other: 'Fine, fine. You cracked me in {count} of 6.' },
    lost: 'Locked tight. It was {answer}. See you tomorrow.',
    wonTitle: 'Cracked', lostTitle: 'Still locked',
    nextIn: 'Next lock in {time}', home: 'Back to QuikFlip',
    network: 'I can’t reach the server. Check your connection and try again.',
    jammed: 'Something jammed on the server. Try again in a moment.',
    retry: 'Try again',
    inkSet: 'Ink set', 'ink.tangerine': 'Tangerine & Navy', 'ink.forest': 'Forest & Coral', 'ink.newsprint': 'Newsprint',
    hardMode: 'Hard mode',
    hardModeHelp: 'Letters you have revealed must be used in later guesses. A change applies from your next new word.',
    saveFailed: 'That setting could not be saved. Try again.',
    'sfx.g': 'click', 'sfx.y': 'rattle', 'sfx.x': '…',
  },
  fr: {
    back: 'Retour à QuikFlip', help: 'Comment jouer', settings: 'Réglages', close: 'Fermer', guesses: 'Essais',
    lockNo: 'Cadenas n° {n}', tumblers: 'Goupilles', pinsLabel: 'Lettres verrouillées : {list}', unknown: 'inconnue',
    enter: 'Crocheter', enterLabel: 'Valider l’essai', delete: 'Effacer',
    rules: 'Tape un mot de cinq lettres et appuie sur Crocheter. Vert : bonne lettre, bonne place. Jaune : la lettre est dans le mot, mais ailleurs. Gris : elle n’y est pas. Tu as six essais.',
    start: 'Cadenas {n}. Cinq lettres, six essais. Je parie que tu ne m’ouvriras pas.',
    resume: { one: 'Tu reviens ? Il te reste 1 essai.', other: 'Tu reviens ? Il te reste {count} essais.' },
    panic: { one: 'Hé, arrête ça ! 1 lettre verrouillée.', other: 'Hé, arrête ça ! {count} lettres verrouillées.' },
    nervous: { one: 'Hum. 1 lettre a fait clic. Coup de chance.', other: 'Hum. {count} lettres ont fait clic. Coup de chance.' },
    smugNear: { one: 'Pas loin, mais mauvaise place. 1 lettre se balade.', other: 'Pas loin, mais mauvaises places. {count} lettres se baladent.' },
    smugNone: 'Ha. Aucune de ces lettres.',
    last: 'Plus qu’un essai. J’aime mes chances.',
    short: 'Cinq lettres, s’il te plaît. Je suis un professionnel.',
    notWord: '« {word} » ? Je ne connais pas ce mot.',
    hardGreen: 'Mode difficile : la lettre {position} doit être {letter}.',
    hardPresent: 'Mode difficile : tu dois utiliser {letter}.',
    won: { one: 'Bon, d’accord. Tu m’as ouvert du premier coup. Frimeur.', other: 'Bon, d’accord. Tu m’as ouvert en {count} essais sur 6.' },
    lost: 'Bien verrouillé. C’était {answer}. À demain.',
    wonTitle: 'Ouvert', lostTitle: 'Toujours fermé',
    nextIn: 'Prochain cadenas dans {time}', home: 'Retour à QuikFlip',
    network: 'Je n’arrive pas à joindre le serveur. Vérifie ta connexion et réessaie.',
    jammed: 'Quelque chose a coincé sur le serveur. Réessaie dans un instant.',
    retry: 'Réessayer',
    inkSet: 'Jeu d’encres', 'ink.tangerine': 'Mandarine & Marine', 'ink.forest': 'Forêt & Corail', 'ink.newsprint': 'Papier journal',
    hardMode: 'Mode difficile',
    hardModeHelp: 'Les lettres trouvées doivent être réutilisées dans les essais suivants. Un changement s’applique à partir de ton prochain mot.',
    saveFailed: 'Ce réglage n’a pas pu être enregistré. Réessaie.',
    'sfx.g': 'clic', 'sfx.y': 'cliquetis', 'sfx.x': '…',
  },
  nl: {
    back: 'Terug naar QuikFlip', help: 'Spelregels', settings: 'Instellingen', close: 'Sluiten', guesses: 'Pogingen',
    lockNo: 'Slot nr. {n}', tumblers: 'Pinnen', pinsLabel: 'Vergrendelde letters: {list}', unknown: 'onbekend',
    enter: 'Kraken', enterLabel: 'Poging indienen', delete: 'Wissen',
    rules: 'Typ een woord van vijf letters en druk op Kraken. Groen: juiste letter op de juiste plek. Geel: de letter zit in het woord, maar ergens anders. Grijs: de letter zit er niet in. Je hebt zes pogingen.',
    start: 'Slot {n}. Vijf letters, zes pogingen. Wedden dat je me niet kraakt?',
    resume: { one: 'Ben je terug? Je hebt nog 1 poging.', other: 'Ben je terug? Je hebt nog {count} pogingen.' },
    panic: { one: 'Hé, hou op! 1 letter vergrendeld.', other: 'Hé, hou op! {count} letters vergrendeld.' },
    nervous: { one: 'Hm. 1 letter klikte. Geluk gehad.', other: 'Hm. {count} letters klikten. Geluk gehad.' },
    smugNear: { one: 'Warm, maar verkeerde plek. 1 letter rammelt rond.', other: 'Warm, maar verkeerde plekken. {count} letters rammelen rond.' },
    smugNone: 'Ha. Geen enkele van die letters.',
    last: 'Nog één poging. Ik heb er alle vertrouwen in.',
    short: 'Vijf letters, alsjeblieft. Ik ben een professional.',
    notWord: '“{word}”? Dat woord ken ik niet.',
    hardGreen: 'Moeilijke modus: letter {position} moet {letter} zijn.',
    hardPresent: 'Moeilijke modus: je moet {letter} gebruiken.',
    won: { one: 'Goed dan. Je kraakte me in één keer. Opschepper.', other: 'Goed dan. Je kraakte me in {count} van de 6 pogingen.' },
    lost: 'Potdicht. Het was {answer}. Tot morgen.',
    wonTitle: 'Gekraakt', lostTitle: 'Nog op slot',
    nextIn: 'Volgend slot over {time}', home: 'Terug naar QuikFlip',
    network: 'Ik kan de server niet bereiken. Controleer je verbinding en probeer opnieuw.',
    jammed: 'Er liep iets vast op de server. Probeer het zo meteen opnieuw.',
    retry: 'Opnieuw proberen',
    inkSet: 'Inktset', 'ink.tangerine': 'Mandarijn & Marine', 'ink.forest': 'Woud & Koraal', 'ink.newsprint': 'Krantenpapier',
    hardMode: 'Moeilijke modus',
    hardModeHelp: 'Gevonden letters moet je in volgende pogingen gebruiken. Een wijziging geldt vanaf je volgende nieuwe woord.',
    saveFailed: 'Die instelling kon niet worden opgeslagen. Probeer opnieuw.',
    'sfx.g': 'klik', 'sfx.y': 'gerammel', 'sfx.x': '…',
  },
};

/* ---------- Pure helpers ---------- */
const count = (marks, mark) => [...marks].filter(value => value === mark).length;
const isFinished = status => status === 'solved' || status === 'failed';

function keyMarks(guesses) {
  const marks = {};
  guesses.forEach(({ word, marks: wordMarks }) => [...word].forEach((letter, index) => {
    const mark = wordMarks[index];
    if (!marks[letter] || MARK_RANK[mark] > MARK_RANK[marks[letter]]) marks[letter] = mark;
  }));
  return marks;
}

// Which slots are known to be right, across every guess so far.
function lockedSlots(guesses, length) {
  return Array.from({ length }, (_, index) => guesses.find(({ marks }) => marks[index] === 'g')?.word[index] ?? '');
}

function lockNumber(isoDate) {
  const [year, month, day] = isoDate.split('-').map(Number);
  return String(Math.max(1, Math.round((Date.UTC(year, month - 1, day) - LOCK_EPOCH) / DAY_MS) + 1)).padStart(4, '0');
}

/* ---------- DOM ---------- */
const root = document.documentElement;
const sheet = document.getElementById('sheet');
const scene = document.getElementById('scene');
const lock = document.getElementById('lock');
const shackle = document.getElementById('shackle');
const pins = document.getElementById('pins');
const board = document.getElementById('board');
const keys = document.getElementById('keys');
const bubble = document.getElementById('bubble');
const mouth = document.getElementById('mouth');
const sfx = document.getElementById('sfx');
const end = document.getElementById('end');

let t = translator(STRINGS, 'en');
let game = null;
let settings = null;
let current = '';
let busy = false;
let currentMood = 'idle';
let stopCountdown = () => {};
const timeouts = new Set();
const after = (ms, callback) => {
  const id = setTimeout(() => { timeouts.delete(id); callback(); }, ms);
  timeouts.add(id);
};
const restartClass = (element, className) => {
  element.classList.remove(className);
  void element.getBoundingClientRect();
  element.classList.add(className);
};

// Only swaps the mood class, so state classes like is-open survive.
function say(line, mood = currentMood) {
  bubble.textContent = line;
  restartClass(bubble, 'is-new');
  scene.classList.replace(`mood-${currentMood}`, `mood-${mood}`);
  currentMood = mood;
  mouth.setAttribute('d', MOUTHS[mood] ?? MOUTHS.idle);
}

function applyInk(inkSet) {
  root.dataset.palette = inkSet;
}

const tileHtml = (letter, mark, extraClass = '') => `
  <div class="tile ${letter ? 'is-filled' : ''} ${extraClass}" role="gridcell" ${letter ? `data-letter="${letter}"` : ''} ${mark ? `data-mark="${mark}"` : ''}>
    <span class="tile-ink"></span><span class="tile-letter">${letter}</span>
  </div>`;

function rowHtml(rowIndex) {
  const guess = game.guesses[rowIndex];
  if (guess) return [...guess.word].map((letter, index) => tileHtml(letter, guess.marks[index])).join('');
  const isCurrent = rowIndex === game.guesses.length && !isFinished(game.status);
  return Array.from({ length: game.wordLength }, (_, index) => isCurrent
    ? tileHtml(current[index] ?? '', '', index === current.length ? 'is-cursor' : '')
    : tileHtml('', '')).join('');
}

const rowElement = rowIndex => board.querySelector(`[data-row="${rowIndex}"]`);

function renderBoard() {
  board.setAttribute('aria-label', t('guesses'));
  board.innerHTML = Array.from({ length: game.maxGuesses }, (_, rowIndex) =>
    `<div class="row" role="row" data-row="${rowIndex}">${rowHtml(rowIndex)}</div>`).join('');
}

function renderKeys() {
  const marks = keyMarks(game.guesses);
  keys.hidden = isFinished(game.status);
  keys.innerHTML = KEY_ROWS[game.keyboard].map(row => `
    <div class="key-row">${row.map(key => {
      const label = key === BACK
        ? '<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M9 5h11v14H9l-6-7z"/><path d="M12.5 9.5l5 5M17.5 9.5l-5 5"/></svg>'
        : key === ENTER ? t('enter') : key.toUpperCase();
      const name = key === BACK ? t('delete') : key === ENTER ? t('enterLabel') : key.toUpperCase();
      return `<button type="button" class="key ${key.length > 1 ? 'is-wide' : ''}" data-key="${key}" ${marks[key] ? `data-mark="${marks[key]}"` : ''} aria-label="${name}">${label}</button>`;
    }).join('')}</div>`).join('');
}

function renderPins() {
  const slots = lockedSlots(game.guesses, game.wordLength);
  pins.innerHTML = slots.map(letter => `<div class="pin-slot ${letter ? 'is-set' : ''}"><span class="pin">${letter}</span></div>`).join('');
  pins.setAttribute('aria-label', t('pinsLabel', { list: slots.map(letter => letter.toUpperCase() || t('unknown')).join(', ') }));
  shackle.style.setProperty('--lift', slots.filter(Boolean).length * SHACKLE_LIFT_PER_LETTER);
}

function renderStatic() {
  root.lang = game.language;
  document.getElementById('back').setAttribute('aria-label', t('back'));
  document.getElementById('help').setAttribute('aria-label', t('help'));
  document.getElementById('open-settings').setAttribute('aria-label', t('settings'));
  document.getElementById('pins-label').textContent = t('tumblers');
  const [year, month, day] = game.date.split('-').map(Number);
  const date = new Intl.DateTimeFormat(LOCALES[game.language], { day: 'numeric', month: 'long', year: 'numeric' }).format(new Date(year, month - 1, day));
  document.getElementById('dateline').textContent = `${t('lockNo', { n: lockNumber(game.date) })} · ${date}`;
  document.getElementById('footer-note').textContent = `${game.language.toUpperCase()} · ${game.date}`;
}

function soundEffect(text, x, y) {
  const piece = document.createElement('span');
  piece.textContent = text;
  piece.style.cssText = `--x: ${x}%; --y: ${y}%`;
  sfx.append(piece);
  after(SFX_MS, () => piece.remove());
}

// The word's letters and ink dots fly out of the opened lock.
function burstLetters(answer) {
  const inks = ['var(--accent)', 'var(--near)', 'var(--hit)', 'var(--ink)'];
  sfx.innerHTML = Array.from({ length: CONFETTI_COUNT }, (_, index) => {
    const isLetter = index % 3 === 0;
    const angle = (index / CONFETTI_COUNT) * Math.PI * 2;
    const distance = 90 + Math.random() * 120;
    return `<span style="--x: 40%; --y: 22%; --anim: burst; --dur: 1400ms; --dx: ${(Math.cos(angle) * distance).toFixed(0)}px; --dy: ${(Math.sin(angle) * distance - 40).toFixed(0)}px; --r: ${(Math.random() * 360 - 180).toFixed(0)}deg; --c: ${inks[index % inks.length]}; --size: ${isLetter ? '1.6rem' : '0.9rem'}">${isLetter ? answer[index % answer.length] : '●'}</span>`;
  }).join('');
  after(SFX_MS, () => { sfx.innerHTML = ''; });
}

// The lock is your opponent: smug when you miss, nervous as you close in.
function reaction(marks) {
  const greens = count(marks, 'g');
  const yellows = count(marks, 'y');
  if (greens >= 3) return { mood: 'panic', line: t('panic', { count: greens }) };
  if (greens) return { mood: 'nervous', line: t('nervous', { count: greens }) };
  if (yellows) return { mood: 'smug', line: t('smugNear', { count: yellows }) };
  return { mood: 'smug', line: t('smugNone') };
}

function showEnd() {
  const won = game.status === 'solved';
  sheet.classList.toggle('is-won', won);
  scene.classList.toggle('is-open', won);
  end.hidden = false;
  end.innerHTML = `
    <strong>${won ? t('wonTitle') : t('lostTitle')}</strong>
    <p>${won ? t('won', { count: game.guesses.length }) : t('lost', { answer: game.answer.toUpperCase() })}</p>
    <p class="countdown" data-countdown></p>
    <a href="../../">${t('home')}</a>`;
}

function greet() {
  if (game.status === 'solved') {
    say(t('won', { count: game.guesses.length }), 'dizzy');
  } else if (game.status === 'failed') {
    say(t('lost', { answer: game.answer.toUpperCase() }), 'smug');
  } else if (game.status === 'progress') {
    const left = game.maxGuesses - game.guesses.length;
    scene.classList.toggle('is-last', left === 1);
    say(t('resume', { count: left }), reaction(game.guesses.at(-1).marks).mood);
  } else {
    say(t('start', { n: lockNumber(game.date) }), 'smug');
  }
}

function startClock(secondsToReset) {
  stopCountdown();
  stopCountdown = startCountdown(secondsToReset, left => {
    const element = end.querySelector('[data-countdown]');
    if (element) element.textContent = t('nextIn', { time: formatDuration(left) });
  }, () => location.reload());
}

/* ---------- Playing ---------- */
function type(key) {
  if (!game || busy || isFinished(game.status)) return;
  if (key === ENTER) {
    submit();
    return;
  }
  if (key === BACK) current = current.slice(0, -1);
  else if (/^[a-z]$/.test(key) && current.length < game.wordLength) current += key;
  else return;
  const row = rowElement(game.guesses.length);
  row.innerHTML = rowHtml(game.guesses.length);
  if (key !== BACK) row.children[current.length - 1].classList.add('is-typed');
}

function explainError(error) {
  switch (error.code) {
    case 'wrong_length':
    case 'invalid_letters': return t('short');
    case 'not_a_word': return t('notWord', { word: current.toUpperCase() });
    case 'hard_mode_green': return t('hardGreen', error.detail ?? {});
    case 'hard_mode_present': return t('hardPresent', error.detail ?? {});
    case 'network': return t('network');
    default: return t('jammed');
  }
}

async function submit() {
  const rowIndex = game.guesses.length;
  const row = rowElement(rowIndex);
  if (current.length < game.wordLength) {
    restartClass(row, 'is-shake');
    say(t('short'), 'smug');
    return;
  }
  busy = true;
  scene.classList.add('is-picking');
  let next;
  try {
    [next] = await Promise.all([api.guess(current), new Promise(resolve => setTimeout(resolve, PICK_MS))]);
  } catch (error) {
    busy = false;
    scene.classList.remove('is-picking');
    if (error.code === 'game_over') {
      await load();
      return;
    }
    restartClass(row, 'is-shake');
    say(explainError(error), 'smug');
    return;
  }
  const guess = next.guesses[rowIndex];
  current = '';
  [...row.children].forEach((tile, index) => after(index * REVEAL_STAGGER_MS, () => {
    tile.dataset.mark = guess.marks[index];
    tile.classList.remove('is-cursor', 'is-typed');
    tile.classList.add('is-revealing');
    restartClass(lock, 'is-tick');
    soundEffect(t(`sfx.${guess.marks[index]}`), 18 + index * 14, 6);
  }));
  after((game.wordLength - 1) * REVEAL_STAGGER_MS + REVEAL_SETTLE_MS, () => finishReveal(next, row));
}

function finishReveal(next, row) {
  game = next;
  busy = false;
  scene.classList.remove('is-picking');
  lock.classList.remove('is-tick');
  renderKeys();
  renderPins();
  if (game.status === 'solved') {
    [...row.children].forEach((tile, index) => tile.style.setProperty('--i', index));
    row.classList.add('is-win');
    say(t('won', { count: game.guesses.length }), 'dizzy');
    burstLetters(game.answer.toUpperCase());
    showEnd();
    startClock(game.secondsToReset);
    return;
  }
  if (game.status === 'failed') {
    scene.classList.remove('is-last');
    say(t('lost', { answer: game.answer.toUpperCase() }), 'smug');
    showEnd();
    startClock(game.secondsToReset);
    return;
  }
  rowElement(game.guesses.length).innerHTML = rowHtml(game.guesses.length);
  const isLast = game.guesses.length === game.maxGuesses - 1;
  scene.classList.toggle('is-last', isLast);
  const { mood, line } = reaction(game.guesses.at(-1).marks);
  say(isLast ? `${line} ${t('last')}` : line, mood);
}

/* ---------- Dialogs ---------- */
function openHelp() {
  showDialog({ title: t('help'), content: `<p>${t('rules')}</p>`, closeLabel: t('close') });
}

function openSettings() {
  const form = document.createElement('form');
  form.className = 'settings';
  const chosen = settings.letterlock;
  form.innerHTML = `
    <fieldset>
      <legend>${t('inkSet')}</legend>
      ${INK_SETS.map(set => `
        <label class="ink-option">
          <input type="radio" name="ink" value="${set.id}" ${set.id === chosen.inkSet ? 'checked' : ''}>
          <i aria-hidden="true">${set.swatches.map(color => `<b style="--c: ${color}"></b>`).join('')}</i>${t(`ink.${set.id}`)}
        </label>`).join('')}
    </fieldset>
    <label class="toggle"><input type="checkbox" name="hard" ${chosen.hardMode ? 'checked' : ''}>${t('hardMode')}</label>
    <p class="help">${t('hardModeHelp')}</p>
    <p class="settings-error" role="alert" hidden></p>`;
  const errorLine = form.querySelector('.settings-error');
  form.addEventListener('submit', event => event.preventDefault());
  form.addEventListener('change', async () => {
    const previous = settings.letterlock;
    const next = { inkSet: form.elements.ink.value, hardMode: form.elements.hard.checked };
    applyInk(next.inkSet);
    try {
      settings = await api.saveLetterlockSettings(next);
      errorLine.hidden = true;
    } catch {
      applyInk(previous.inkSet);
      form.elements.ink.value = previous.inkSet;
      form.elements.hard.checked = previous.hardMode;
      errorLine.textContent = t('saveFailed');
      errorLine.hidden = false;
    }
  });
  showDialog({ title: t('settings'), content: form, closeLabel: t('close') });
}

/* ---------- Loading ---------- */
async function load() {
  timeouts.forEach(clearTimeout);
  timeouts.clear();
  try {
    [settings, game] = await Promise.all([api.settings(), api.letterlock()]);
  } catch (error) {
    bubble.innerHTML = `${error.code === 'network' ? t('network') : t('jammed')}<br><button type="button" data-retry>${t('retry')}</button>`;
    return;
  }
  t = translator(STRINGS, game.language);
  current = '';
  busy = false;
  applyInk(settings.letterlock.inkSet);
  renderStatic();
  renderBoard();
  renderKeys();
  renderPins();
  end.hidden = true;
  greet();
  if (isFinished(game.status)) showEnd();
  startClock(game.secondsToReset);
}

keys.addEventListener('click', event => {
  const key = event.target.closest('[data-key]');
  if (key) type(key.dataset.key);
});
bubble.addEventListener('click', event => {
  if (event.target.closest('[data-retry]')) load();
});
document.getElementById('help').addEventListener('click', openHelp);
document.getElementById('open-settings').addEventListener('click', () => { if (settings) openSettings(); });

document.addEventListener('keydown', event => {
  if (event.metaKey || event.ctrlKey || event.altKey || document.querySelector('dialog[open]')) return;
  const key = event.key === ENTER || event.key === BACK ? event.key : event.key.toLowerCase();
  if (key === ENTER || key === BACK || /^[a-z]$/.test(key)) {
    event.preventDefault();
    type(key);
  }
});

load();
