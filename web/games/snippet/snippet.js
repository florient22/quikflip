// Snippet "Mixtape": name the song from ever longer clips. The server sends only the part of the song you have
// unlocked; this page decodes it, plays it through a level meter and animates the cassette.
import { api } from '../../js/api.js';
import { LOCALES, translator } from '../../js/i18n.js';
import { showDialog } from '../../js/dialog.js';
import { formatDuration, startCountdown } from '../../js/countdown.js';

const SUGGEST_DELAY_MS = 180;
const MIN_SUGGEST_CHARS = 2;
const CHEW_MS = 1900;
const FF_MS = 900;
const STAR_COUNT = 14;
const STAR_MS = 1300;
const NEEDLE_REST_DEG = -48;
const NEEDLE_SWING_DEG = 96;
const TAPE_FULL = 30;
const TAPE_EMPTY = 14;
const LABEL_MAX_CHARS = 22;
const LEVEL_GAIN = 3.2;
// What a suggestion puts between title and artist; the server splits a guess on it.
const TITLE_ARTIST_SEPARATOR = ' — ';
const MODES = ['mix', 'random', 'genre'];
const GENRES = ['pop', 'hiphop', 'rock', 'dance', 'rnb', 'alternative', 'electro', 'metal', 'soul', 'latin', 'chanson', 'dutch'];
const CLIP_LENGTHS = ['easy', 'normal', 'hard'];
const MOUTHS = {
  idle: 'M160 196 Q180 208 200 196',
  happy: 'M156 192 Q180 222 204 192 Z',
  chewed: 'M160 202 Q170 194 180 202 Q190 210 200 202',
  sad: 'M162 206 Q180 194 198 206',
  ff: 'M168 200 H192',
};

const STRINGS = {
  en: {
    back: 'Back to QuikFlip', help: 'How to play', settings: 'Settings', close: 'Close',
    rules: 'Press play and name the song. Each skip or wrong guess unlocks a longer clip, and you have six chances. Pick a suggestion or type the title (add “ — artist” if two songs share a title). Choose the daily mix, a random genre every day, or one genre in the settings.',
    mixtape: 'Mixtape of the day', genreOfDay: 'Today’s genre: {genre}', mystery: 'MYSTERY MIX',
    jcard: 'Tracklist guesses', skipped: 'skipped', play: 'Play', playFull: 'Full clip', skip: 'Skip', counter: 'Counter',
    guessLabel: 'Your guess', placeholder: 'Name that tune…', guess: 'Guess',
    clipOf: 'Clip {n}/{total} · {seconds}s', fullClip: 'Full clip · {seconds}s',
    seconds: { one: '1 second', other: '{count} seconds' },
    start: 'Fresh tape in the deck. Hit play!', resume: 'Back for more? Clip {n} is waiting.',
    listen: 'Listen closely…', ff: 'Fast-forwarding… Clip {n}: {seconds}.', chewed: 'Ugh, I chewed the tape. Not that one. Clip {n}: {seconds}.',
    right: { one: 'That’s my jam! Got it on the first clip!', other: 'That’s my jam! Got it on clip {count}.' },
    over: 'Side A’s over. It was “{title}” by {artist}.',
    solvedStatus: '“{title}” by {artist}. Press play for the full clip.', failedStatus: 'Press play to hear the full clip.',
    hint: 'Each skip or wrong guess unlocks a longer clip.',
    waiting: 'Today’s tape is still being recorded. Check back soon.',
    album: 'Album: {album}', deezer: 'Listen on Deezer', nextIn: 'Next tape in {time}', empty: 'Type a song first.',
    audioFailed: 'The tape won’t play. Check your connection and try again.', noAudio: 'This browser can’t play audio clips.',
    mode: 'Songs', 'mode.mix': 'Daily mix', 'mode.mix.help': 'A popular song from any genre',
    'mode.random': 'Random genre', 'mode.random.help': 'A different genre every day, shown as a hint',
    'mode.genre': 'One genre', 'mode.genre.help': 'Always the genre below',
    genre: 'Genre', clips: 'Clip length', 'clips.easy': 'Easy: longer clips', 'clips.normal': 'Normal', 'clips.hard': 'Hard: shorter clips',
    settingsHelp: 'Changes apply right away. Each mode and genre has its own song every day.',
    saveFailed: 'That setting could not be saved. Try again.',
    'genre.pop': 'Pop', 'genre.hiphop': 'Hip-hop', 'genre.rock': 'Rock', 'genre.dance': 'Dance', 'genre.rnb': 'R&B', 'genre.alternative': 'Alternative',
    'genre.electro': 'Electro', 'genre.metal': 'Metal', 'genre.soul': 'Soul & funk', 'genre.latin': 'Latin', 'genre.chanson': 'French chanson', 'genre.dutch': 'Dutch-language',
    network: 'I can’t reach the server. Check your connection and try again.',
    jammed: 'Something jammed on the server. Try again in a moment.', retry: 'Try again',
  },
  fr: {
    back: 'Retour à QuikFlip', help: 'Comment jouer', settings: 'Réglages', close: 'Fermer',
    rules: 'Appuie sur lecture et trouve la chanson. Chaque passe ou mauvaise réponse débloque un extrait plus long, et tu as six chances. Choisis une suggestion ou tape le titre (ajoute « — artiste » si deux chansons ont le même titre). Choisis le mix du jour, un genre au hasard chaque jour ou un seul genre dans les réglages.',
    mixtape: 'La cassette du jour', genreOfDay: 'Genre du jour : {genre}', mystery: 'MIX MYSTÈRE',
    jcard: 'Mes propositions', skipped: 'passé', play: 'Lecture', playFull: 'Extrait complet', skip: 'Passer', counter: 'Compteur',
    guessLabel: 'Ta réponse', placeholder: 'C’est quoi, ce morceau ?', guess: 'Valider',
    clipOf: 'Extrait {n}/{total} · {seconds} s', fullClip: 'Extrait complet · {seconds} s',
    seconds: { one: '1 seconde', other: '{count} secondes' },
    start: 'Une nouvelle cassette dans le lecteur. Appuie sur lecture !', resume: 'Tu reviens ? L’extrait {n} t’attend.',
    listen: 'Écoute bien…', ff: 'Avance rapide… Extrait {n} : {seconds}.', chewed: 'Aïe, j’ai mâché la bande. Pas celle-là. Extrait {n} : {seconds}.',
    right: { one: 'C’est mon morceau ! Trouvé dès le premier extrait !', other: 'C’est mon morceau ! Trouvé à l’extrait {count}.' },
    over: 'Face A terminée. C’était « {title} » de {artist}.',
    solvedStatus: '« {title} » de {artist}. Appuie sur lecture pour l’extrait complet.', failedStatus: 'Appuie sur lecture pour écouter l’extrait complet.',
    hint: 'Chaque passe ou mauvaise réponse débloque un extrait plus long.',
    waiting: 'La cassette du jour est encore en enregistrement. Reviens bientôt.',
    album: 'Album : {album}', deezer: 'Écouter sur Deezer', nextIn: 'Prochaine cassette dans {time}', empty: 'Tape d’abord une chanson.',
    audioFailed: 'La cassette ne veut pas jouer. Vérifie ta connexion et réessaie.', noAudio: 'Ce navigateur ne peut pas lire les extraits audio.',
    mode: 'Chansons', 'mode.mix': 'Mix du jour', 'mode.mix.help': 'Une chanson populaire, tous genres confondus',
    'mode.random': 'Genre au hasard', 'mode.random.help': 'Un genre différent chaque jour, donné en indice',
    'mode.genre': 'Un seul genre', 'mode.genre.help': 'Toujours le genre ci-dessous',
    genre: 'Genre', clips: 'Longueur des extraits', 'clips.easy': 'Facile : extraits plus longs', 'clips.normal': 'Normal', 'clips.hard': 'Difficile : extraits plus courts',
    settingsHelp: 'Les changements s’appliquent tout de suite. Chaque mode et chaque genre a sa propre chanson chaque jour.',
    saveFailed: 'Ce réglage n’a pas pu être enregistré. Réessaie.',
    'genre.pop': 'Pop', 'genre.hiphop': 'Hip-hop', 'genre.rock': 'Rock', 'genre.dance': 'Dance', 'genre.rnb': 'R&B', 'genre.alternative': 'Alternative',
    'genre.electro': 'Électro', 'genre.metal': 'Metal', 'genre.soul': 'Soul et funk', 'genre.latin': 'Latino', 'genre.chanson': 'Chanson française', 'genre.dutch': 'Musique néerlandophone',
    network: 'Je n’arrive pas à joindre le serveur. Vérifie ta connexion et réessaie.',
    jammed: 'Quelque chose a coincé sur le serveur. Réessaie dans un instant.', retry: 'Réessayer',
  },
  nl: {
    back: 'Terug naar QuikFlip', help: 'Spelregels', settings: 'Instellingen', close: 'Sluiten',
    rules: 'Druk op play en raad het nummer. Elke keer dat je overslaat of fout raadt, wordt het fragment langer, en je hebt zes kansen. Kies een suggestie of typ de titel (voeg “ — artiest” toe als twee nummers dezelfde titel hebben). Kies in de instellingen de dagmix, elke dag een willekeurig genre of één genre.',
    mixtape: 'Mixtape van de dag', genreOfDay: 'Genre van vandaag: {genre}', mystery: 'MYSTERIEMIX',
    jcard: 'Mijn gokken', skipped: 'overgeslagen', play: 'Play', playFull: 'Volledig fragment', skip: 'Overslaan', counter: 'Teller',
    guessLabel: 'Jouw gok', placeholder: 'Welk nummer is dit?', guess: 'Raden',
    clipOf: 'Fragment {n}/{total} · {seconds} s', fullClip: 'Volledig fragment · {seconds} s',
    seconds: { one: '1 seconde', other: '{count} seconden' },
    start: 'Een verse cassette in de speler. Druk op play!', resume: 'Terug voor meer? Fragment {n} wacht op je.',
    listen: 'Goed luisteren…', ff: 'Vooruitspoelen… Fragment {n}: {seconds}.', chewed: 'Oei, ik heb de band opgegeten. Niet dat nummer. Fragment {n}: {seconds}.',
    right: { one: 'Mijn favoriet! Geraden bij het eerste fragment!', other: 'Mijn favoriet! Geraden bij fragment {count}.' },
    over: 'Kant A is afgelopen. Het was “{title}” van {artist}.',
    solvedStatus: '“{title}” van {artist}. Druk op play voor het volledige fragment.', failedStatus: 'Druk op play voor het volledige fragment.',
    hint: 'Elke keer overslaan of fout raden maakt het fragment langer.',
    waiting: 'De cassette van vandaag wordt nog opgenomen. Kom straks terug.',
    album: 'Album: {album}', deezer: 'Luister op Deezer', nextIn: 'Volgende cassette over {time}', empty: 'Typ eerst een nummer.',
    audioFailed: 'De cassette wil niet spelen. Controleer je verbinding en probeer opnieuw.', noAudio: 'Deze browser kan geen audiofragmenten afspelen.',
    mode: 'Nummers', 'mode.mix': 'Dagmix', 'mode.mix.help': 'Een populair nummer uit elk genre',
    'mode.random': 'Willekeurig genre', 'mode.random.help': 'Elke dag een ander genre, als hint getoond',
    'mode.genre': 'Eén genre', 'mode.genre.help': 'Altijd het genre hieronder',
    genre: 'Genre', clips: 'Lengte van de fragmenten', 'clips.easy': 'Makkelijk: langere fragmenten', 'clips.normal': 'Normaal', 'clips.hard': 'Moeilijk: kortere fragmenten',
    settingsHelp: 'Wijzigingen gelden meteen. Elke modus en elk genre heeft elke dag een eigen nummer.',
    saveFailed: 'Die instelling kon niet worden opgeslagen. Probeer opnieuw.',
    'genre.pop': 'Pop', 'genre.hiphop': 'Hiphop', 'genre.rock': 'Rock', 'genre.dance': 'Dance', 'genre.rnb': 'R&B', 'genre.alternative': 'Alternatief',
    'genre.electro': 'Electro', 'genre.metal': 'Metal', 'genre.soul': 'Soul en funk', 'genre.latin': 'Latin', 'genre.chanson': 'Franse chanson', 'genre.dutch': 'Nederlandstalig',
    network: 'Ik kan de server niet bereiken. Controleer je verbinding en probeer opnieuw.',
    jammed: 'Er liep iets vast op de server. Probeer het zo meteen opnieuw.', retry: 'Opnieuw proberen',
  },
};

const escapeHtml = text => String(text).replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]);
const restartClass = (element, className) => { element.classList.remove(className); void element.getBoundingClientRect(); element.classList.add(className); };
const truncate = (text, length) => (text.length > length ? `${text.slice(0, length - 1)}…` : text);

/* ---------- Audio: decodes the clip the server allows and plays it through a level meter ---------- */
class ClipPlayer {
  constructor() {
    this.context = null;
    this.analyser = null;
    this.samples = null;
    this.buffer = null;
    this.bufferKey = '';
    this.source = null;
    this.finish = null;
  }

  ensureContext() {
    if (!this.context) {
      const AudioContextClass = window.AudioContext || window.webkitAudioContext;
      if (!AudioContextClass) throw new Error('no-audio');
      this.context = new AudioContextClass();
      this.analyser = this.context.createAnalyser();
      this.analyser.fftSize = 512;
      this.analyser.connect(this.context.destination);
      this.samples = new Float32Array(this.analyser.fftSize);
    }
    this.context.resume();
    return this.context;
  }

  /** Fetches and decodes the clip again only when it changed (a longer clip, or the full one at the end). */
  async load(key) {
    if (this.bufferKey === key && this.buffer) return;
    const response = await fetch(api.snippetClipUrl, { cache: 'no-store' });
    if (!response.ok) throw new Error(`clip: ${response.status}`);
    this.buffer = await this.context.decodeAudioData(await response.arrayBuffer());
    this.bufferKey = key;
  }

  forget() {
    this.buffer = null;
    this.bufferKey = '';
  }

  // Root-mean-square level of what is playing right now, 0 to about 1.
  level() {
    if (!this.analyser) return 0;
    this.analyser.getFloatTimeDomainData(this.samples);
    let sum = 0;
    for (const sample of this.samples) sum += sample * sample;
    return Math.min(1, Math.sqrt(sum / this.samples.length) * LEVEL_GAIN);
  }

  /** Plays the first `seconds` of the loaded clip; resolves when it ends or is stopped. */
  play(seconds) {
    this.stop();
    const source = this.context.createBufferSource();
    source.buffer = this.buffer;
    source.connect(this.analyser);
    this.source = source;
    return new Promise(resolve => {
      this.finish = resolve;
      source.onended = () => this.stop();
      source.start(0, 0, Math.min(seconds, this.buffer.duration));
    });
  }

  stop() {
    if (this.source) {
      this.source.onended = null;
      try { this.source.stop(); } catch { /* already stopped */ }
      this.source.disconnect();
      this.source = null;
    }
    const finish = this.finish;
    this.finish = null;
    finish?.();
  }
}

/* ---------- DOM ---------- */
const wrap = document.getElementById('tape-wrap');
const bubble = document.getElementById('bubble');
const mouth = document.getElementById('mouth');
const labelText = document.getElementById('label-text');
const tapeLeft = document.getElementById('tape-left');
const tapeRight = document.getElementById('tape-right');
const needle = document.getElementById('needle');
const counter = document.getElementById('counter');
const playButton = document.getElementById('play-btn');
const skipButton = document.getElementById('skip-btn');
const clipBar = document.getElementById('clip-bar');
const historyList = document.getElementById('history');
const reveal = document.getElementById('reveal');
const form = document.getElementById('guess-form');
const input = document.getElementById('guess-input');
const suggest = document.getElementById('suggest');
const statusLine = document.getElementById('status');
const sfx = document.getElementById('sfx');
const dateline = document.getElementById('dateline');

const player = new ClipPlayer();
let t = translator(STRINGS, 'en');
let lang = 'en';
let game = null;
let settings = null;
let busy = false;
let playing = false;
let playStarted = 0;
let currentMood = 'idle';
let meterFrame = 0;
let suggestTimer = 0;
let suggestRequest = 0;
let loadingClip = false;
let stopCountdown = () => {};
const after = (ms, callback) => setTimeout(callback, ms);

const finished = () => game.status === 'solved' || game.status === 'failed';
const clipSeconds = () => (finished() ? game.fullSeconds : game.clips[game.stage]);
const clipKey = () => `${game.date}:${game.variant}:${finished() ? 'full' : game.clips[game.stage]}`;
const secondsText = seconds => t('seconds', { count: seconds });

function setMood(mood) {
  wrap.classList.replace(`mood-${currentMood}`, `mood-${mood}`);
  currentMood = mood;
  mouth.setAttribute('d', MOUTHS[mood] ?? MOUTHS.idle);
  mouth.setAttribute('class', mood === 'happy' ? 'mouth-fill' : 'mouth');
}

function say(line, mood) {
  bubble.textContent = line;
  restartClass(bubble, 'is-new');
  if (mood) setMood(mood);
}

function setCounter(tenths) {
  [...String(Math.min(999, tenths)).padStart(3, '0')].forEach((digit, index) => { counter.children[index].textContent = digit; });
}

// VU needle and the cassette's little beat bounce follow the real audio level.
function meter() {
  const level = playing ? player.level() : 0;
  needle.style.setProperty('--needle', `${(NEEDLE_REST_DEG + level * NEEDLE_SWING_DEG).toFixed(1)}deg`);
  wrap.style.setProperty('--beat', level.toFixed(3));
  if (playing) {
    setCounter(Math.floor((performance.now() - playStarted) / 100));
    meterFrame = requestAnimationFrame(meter);
  }
}

/* ---------- Rendering ---------- */
function formatDate(isoDate) {
  const [year, month, day] = isoDate.split('-').map(Number);
  return new Intl.DateTimeFormat(LOCALES[lang], { day: 'numeric', month: 'short' }).format(new Date(year, month - 1, day));
}

function renderStatic() {
  document.documentElement.lang = lang;
  const label = (id, key) => document.getElementById(id).setAttribute('aria-label', t(key));
  label('back', 'back'); label('help', 'help'); label('open-settings', 'settings');
  document.getElementById('jcard-title').textContent = t('jcard');
  document.getElementById('counter-label').textContent = t('counter');
  document.getElementById('skip-label').textContent = t('skip');
  document.getElementById('guess-label').textContent = t('guessLabel');
  document.getElementById('guess-btn').textContent = t('guess');
  input.placeholder = t('placeholder');
}

function render() {
  const waiting = game.status === 'waiting';
  const done = !waiting && finished();
  const genre = game.genre ? t(`genre.${game.genre}`) : null;
  dateline.textContent = `${genre ? t('genreOfDay', { genre }) : t('mixtape')} · ${formatDate(game.date)}`;
  labelText.textContent = truncate((done ? game.answer.title : genre ?? t('mystery')).toUpperCase(), LABEL_MAX_CHARS);
  // Tape winds from the left reel to the right one as longer clips unlock.
  const progress = done ? 1 : game.stage / (game.clips.length - 1);
  tapeLeft.setAttribute('r', (TAPE_FULL - progress * (TAPE_FULL - TAPE_EMPTY)).toFixed(1));
  tapeRight.setAttribute('r', (TAPE_EMPTY + progress * (TAPE_FULL - TAPE_EMPTY)).toFixed(1));
  clipBar.innerHTML = game.clips.map((seconds, index) => {
    const state = done || index < game.stage ? 'is-on' : index === game.stage ? 'is-current' : '';
    return `<span class="${state}" style="--len: ${seconds}"></span>`;
  }).join('') + `<b class="clip-label">${escapeHtml(done ? t('fullClip', { seconds: game.fullSeconds }) : t('clipOf', { n: game.stage + 1, total: game.clips.length, seconds: game.clips[game.stage] }))}</b>`;
  const lines = game.guesses.map(guess => {
    if (guess.kind === 'skip') return `<li class="is-skip">${escapeHtml(t('skipped'))}</li>`;
    return guess.kind === 'right' ? `<li class="is-right"><span>${escapeHtml(guess.text)}</span></li>` : `<li class="is-wrong">${escapeHtml(guess.text)}</li>`;
  });
  if (!done && !waiting) lines.push('<li class="is-pending">…</li>');
  historyList.innerHTML = lines.join('');
  document.getElementById('play-label').textContent = done ? t('playFull') : t('play');
  playButton.disabled = waiting;
  skipButton.disabled = waiting || done;
  form.hidden = waiting || done;
  renderReveal(done);
}

function renderReveal(done) {
  reveal.hidden = !done;
  if (!done) return;
  const { title, artist, album, link } = game.answer;
  const cover = game.hasCover ? `<img src="${api.snippetCoverUrl}?for=${encodeURIComponent(`${game.date}-${game.variant}`)}" alt="">` : '';
  reveal.innerHTML = `${cover}<b>${escapeHtml(title)}</b><span>${escapeHtml(artist)}</span>
    <span>${escapeHtml(t('album', { album }))} · <a href="${escapeHtml(link)}" target="_blank" rel="noopener noreferrer">${escapeHtml(t('deezer'))}</a></span>
    <span class="countdown" data-countdown></span>`;
}

function burstStars() {
  const colors = ['var(--orange)', 'var(--cream)', 'var(--brick)', 'var(--pencil)', 'var(--shell)'];
  sfx.innerHTML = Array.from({ length: STAR_COUNT }, (_, index) => {
    const angle = (index / STAR_COUNT) * Math.PI * 2;
    const distance = 110 + Math.random() * 90;
    return `<span style="--x: 47%; --y: 40%; --s: ${(14 + Math.random() * 16).toFixed(0)}px; --c: ${colors[index % colors.length]}; --dx: ${(Math.cos(angle) * distance).toFixed(0)}px; --dy: ${(Math.sin(angle) * distance).toFixed(0)}px"></span>`;
  }).join('');
  after(STAR_MS, () => { sfx.innerHTML = ''; });
}

/* ---------- Actions ---------- */
function explain(error) {
  if (error.code === 'empty_guess') return t('empty');
  return error.code === 'network' ? t('network') : t('jammed');
}

async function play() {
  if (!game || game.status === 'waiting' || loadingClip) return;
  if (playing) {
    player.stop();
    return;
  }
  try {
    player.ensureContext();
  } catch {
    statusLine.textContent = t('noAudio');
    return;
  }
  playButton.classList.add('is-down');
  loadingClip = true;
  try {
    await player.load(clipKey());
  } catch {
    playButton.classList.remove('is-down');
    say(t('audioFailed'), 'sad');
    return;
  } finally {
    loadingClip = false;
  }
  playing = true;
  playStarted = performance.now();
  wrap.classList.add('is-playing');
  if (!finished()) say(t('listen'), 'idle');
  meter();
  await player.play(clipSeconds());
  playing = false;
  wrap.classList.remove('is-playing');
  playButton.classList.remove('is-down');
  meter();
}

// After a skip or a wrong guess: the next clip, or the end of the tape.
function afterMiss(lineKey, mood, settleMs) {
  render();
  if (finished()) {
    say(t('over', { title: game.answer.title, artist: game.answer.artist }), 'sad');
    statusLine.textContent = t('failedStatus');
    startNextCountdown();
    return;
  }
  say(t(lineKey, { n: game.stage + 1, seconds: secondsText(game.clips[game.stage]) }), mood);
  after(settleMs, () => { if (currentMood === mood) setMood('idle'); });
}

async function act(request) {
  if (busy) return null;
  busy = true;
  try {
    return await request();
  } catch (error) {
    say(explain(error), 'sad');
    return null;
  } finally {
    busy = false;
  }
}

async function skip() {
  if (!game || finished()) return;
  player.stop();
  const next = await act(() => api.snippetSkip());
  if (!next) return;
  game = next;
  afterMiss('ff', 'ff', FF_MS);
}

async function guess(text) {
  if (!text) {
    say(t('empty'), 'sad');
    return;
  }
  player.stop();
  const next = await act(() => api.snippetGuess(text));
  if (!next) return;
  game = next;
  if (game.status === 'solved') {
    render();
    const clip = game.guesses.length;
    say(t('right', { count: clip }), 'happy');
    statusLine.textContent = t('solvedStatus', { title: game.answer.title, artist: game.answer.artist });
    burstStars();
    startNextCountdown();
    return;
  }
  afterMiss('chewed', 'chewed', CHEW_MS);
}

/* ---------- Suggestions from the song catalog ---------- */
function hideSuggestions() {
  suggest.hidden = true;
  suggest.innerHTML = '';
}

async function loadSuggestions(query) {
  const request = ++suggestRequest;
  let songs;
  try {
    songs = await api.snippetSuggest(query);
  } catch {
    return;
  }
  // A newer keystroke already asked again; drop this answer.
  if (request !== suggestRequest) return;
  suggest.innerHTML = songs.map(song => {
    const value = `${song.title}${TITLE_ARTIST_SEPARATOR}${song.artist}`;
    return `<li><button type="button" data-pick="${escapeHtml(value)}">${escapeHtml(song.title)} <small>${escapeHtml(song.artist)}</small></button></li>`;
  }).join('');
  suggest.hidden = songs.length === 0;
}

input.addEventListener('input', () => {
  clearTimeout(suggestTimer);
  const query = input.value.trim();
  if (query.length < MIN_SUGGEST_CHARS) {
    suggestRequest += 1;
    hideSuggestions();
    return;
  }
  suggestTimer = setTimeout(() => loadSuggestions(query), SUGGEST_DELAY_MS);
});
input.addEventListener('keydown', event => {
  if (event.key === 'ArrowDown' && !suggest.hidden) {
    event.preventDefault();
    suggest.querySelector('button')?.focus();
  } else if (event.key === 'Escape') {
    hideSuggestions();
  }
});
suggest.addEventListener('keydown', event => {
  const buttons = [...suggest.querySelectorAll('button')];
  const index = buttons.indexOf(document.activeElement);
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault();
    const next = index + (event.key === 'ArrowDown' ? 1 : -1);
    if (next < 0) input.focus();
    else buttons[Math.min(next, buttons.length - 1)].focus();
  } else if (event.key === 'Escape') {
    hideSuggestions();
    input.focus();
  }
});
suggest.addEventListener('click', event => {
  const pick = event.target.closest('[data-pick]');
  if (!pick) return;
  input.value = pick.dataset.pick;
  hideSuggestions();
  input.focus();
});
document.addEventListener('click', event => {
  if (!form.contains(event.target)) hideSuggestions();
});
form.addEventListener('submit', event => {
  event.preventDefault();
  const text = input.value.trim();
  input.value = '';
  suggestRequest += 1;
  hideSuggestions();
  guess(text);
});
playButton.addEventListener('click', play);
skipButton.addEventListener('click', skip);

/* ---------- Dialogs ---------- */
function openSettings() {
  const settingsForm = document.createElement('form');
  settingsForm.className = 'settings';
  const current = settings.snippet;
  settingsForm.innerHTML = `
    <fieldset>
      <legend>${escapeHtml(t('mode'))}</legend>
      ${MODES.map(mode => `<label class="option"><input type="radio" name="mode" value="${mode}" ${current.mode === mode ? 'checked' : ''}><span>${escapeHtml(t(`mode.${mode}`))}<small>${escapeHtml(t(`mode.${mode}.help`))}</small></span></label>`).join('')}
    </fieldset>
    <label class="option"><span>${escapeHtml(t('genre'))}</span>
      <select name="genre" ${current.mode === 'genre' ? '' : 'disabled'}>
        ${GENRES.map(genre => `<option value="${genre}" ${current.genre === genre ? 'selected' : ''}>${escapeHtml(t(`genre.${genre}`))}</option>`).join('')}
      </select>
    </label>
    <fieldset>
      <legend>${escapeHtml(t('clips'))}</legend>
      ${CLIP_LENGTHS.map(length => `<label class="option"><input type="radio" name="clips" value="${length}" ${current.clips === length ? 'checked' : ''}>${escapeHtml(t(`clips.${length}`))}</label>`).join('')}
    </fieldset>
    <p class="help">${escapeHtml(t('settingsHelp'))}</p>
    <p class="settings-error" role="alert" hidden></p>`;
  const errorLine = settingsForm.querySelector('.settings-error');
  const { elements } = settingsForm;
  settingsForm.addEventListener('submit', event => event.preventDefault());
  settingsForm.addEventListener('change', async () => {
    const previous = settings.snippet;
    elements.genre.disabled = elements.mode.value !== 'genre';
    try {
      settings = await api.saveSnippetSettings({ mode: elements.mode.value, genre: elements.genre.value, clips: elements.clips.value });
      errorLine.hidden = true;
      await reloadGame();
    } catch {
      elements.mode.value = previous.mode;
      elements.genre.value = previous.genre;
      elements.clips.value = previous.clips;
      elements.genre.disabled = previous.mode !== 'genre';
      errorLine.textContent = t('saveFailed');
      errorLine.hidden = false;
    }
  });
  showDialog({ title: t('settings'), content: settingsForm, closeLabel: t('close') });
}
document.getElementById('help').addEventListener('click', () => showDialog({ title: t('help'), content: `<p>${escapeHtml(t('rules'))}</p>`, closeLabel: t('close') }));
document.getElementById('open-settings').addEventListener('click', () => { if (settings) openSettings(); });

/* ---------- Loading ---------- */
function startNextCountdown() {
  stopCountdown();
  stopCountdown = startCountdown(game.secondsToReset, left => {
    const element = reveal.querySelector('[data-countdown]');
    if (element) element.textContent = t('nextIn', { time: formatDuration(left) });
  }, () => location.reload());
}

function greet() {
  sfx.innerHTML = '';
  statusLine.textContent = '';
  if (game.status === 'waiting') {
    say(t('waiting'), 'sad');
  } else if (game.status === 'solved') {
    say(t('right', { count: game.guesses.length }), 'happy');
    statusLine.textContent = t('solvedStatus', { title: game.answer.title, artist: game.answer.artist });
  } else if (game.status === 'failed') {
    say(t('over', { title: game.answer.title, artist: game.answer.artist }), 'sad');
    statusLine.textContent = t('failedStatus');
  } else {
    say(game.status === 'new' ? t('start') : t('resume', { n: game.stage + 1 }), 'idle');
    statusLine.textContent = t('hint');
  }
}

// Called after a settings change: another mode or genre means another song.
async function reloadGame() {
  player.stop();
  player.forget();
  game = await api.snippet();
  render();
  greet();
  startNextCountdown();
}

async function load() {
  try {
    [settings, game] = await Promise.all([api.settings(), api.snippet()]);
  } catch (error) {
    bubble.innerHTML = `${escapeHtml(error.code === 'network' ? t('network') : t('jammed'))} <button type="button" data-retry>${escapeHtml(t('retry'))}</button>`;
    return;
  }
  lang = settings.app.language;
  t = translator(STRINGS, lang);
  renderStatic();
  setCounter(0);
  render();
  greet();
  meter();
  // The countdown also reloads the page at midnight for the new song.
  startNextCountdown();
}
bubble.addEventListener('click', event => { if (event.target.closest('[data-retry]')) load(); });

load();
