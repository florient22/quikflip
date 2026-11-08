import { api } from '../../js/api.js';
import { translator } from '../../js/i18n.js';
import { showDialog } from '../../js/dialog.js';
import { formatDuration, startCountdown } from '../../js/countdown.js';

// Rarity: the floor each tier builds (must match the server's points).
const TIERS = [
  { meters: 30, points: 10, color: 'var(--tier-1)' },
  { meters: 45, points: 25, color: 'var(--tier-2)' },
  { meters: 65, points: 50, color: 'var(--tier-3)' },
  { meters: 90, points: 80, color: 'var(--tier-4)' },
  { meters: 120, points: 100, color: 'var(--tier-5)' },
];
const PX_PER_METER = 1.2;
const GROUND_PX = 70;
const WORLD_PX = 1500;
const CAMERA_TOP_SHARE = 0.5;       // keep the tower top around the middle of the screen
const DROP_PX = 110;
const COUNT_UP_MS = 900;
const URGENT_SECONDS = 6;
const DROP_MS = 520;
const NEXT_PROMPT_DELAY_MS = 1900;
const TIMEOUT_NEXT_DELAY_MS = 2800;
const CRATE_RESET_MS = 900;
const LANDMARKS = [
  { meters: 50, x: '20%', w: 26, shape: 'polygon(36% 100%, 64% 100%, 58% 18%, 70% 18%, 70% 9%, 50% 0, 30% 9%, 30% 18%, 42% 18%)' },
  { meters: 96, x: '30%', w: 32, shape: 'polygon(20% 100%, 80% 100%, 80% 30%, 88% 26%, 50% 0, 12% 26%, 20% 30%)' },
  { meters: 139, x: '2%', w: 230, shape: 'polygon(0 100%, 100% 100%, 50% 0)' },
  { meters: 330, x: '34%', w: 120, shape: 'polygon(0 100%, 18% 100%, 35% 70%, 65% 70%, 82% 100%, 100% 100%, 62% 45%, 54% 8%, 51% 0, 49% 0, 46% 8%, 38% 45%)' },
  { meters: 381, x: '12%', w: 80, shape: 'polygon(0 100%, 100% 100%, 100% 30%, 82% 30%, 82% 16%, 64% 16%, 58% 6%, 51% 0, 49% 0, 42% 6%, 36% 16%, 18% 16%, 18% 30%, 0 30%)' },
  { meters: 828, x: '24%', w: 70, shape: 'polygon(0 100%, 100% 100%, 82% 70%, 70% 45%, 60% 22%, 52% 0, 48% 0, 40% 22%, 30% 45%, 18% 70%)' },
];
// Sky colours from the ground up: [height share, top colour, bottom colour].
const SKY_DAY = [[0, '#8fd3ff', '#fff3d6'], [0.35, '#ff9f80', '#ffd6a3'], [0.7, '#5b3f9a', '#ff8f7a'], [1, '#141238', '#4b2f7a']];
// At night the sky reaches pure black at the top (OLED-friendly).
const SKY_NIGHT = [[0, '#0a1030', '#1e1640'], [0.35, '#120a2a', '#3a1846'], [0.7, '#04020e', '#160c2c'], [1, '#000000', '#04020e']];
const CLOUDS = [[8, 300, 120, 18], [62, 340, 90, 24], [30, 420, 150, 20], [78, 470, 110, 28], [14, 560, 100, 22]];

const STRINGS = {
  en: {
    back: 'Back to QuikFlip', help: 'How to play', settings: 'Settings', close: 'Close',
    rules: 'Seven prompts a day. Each one accepts lots of answers: type one before the crane gets tired. Every accepted answer adds a floor to your tower, and rarer answers build bigger, fancier floors. A wrong answer costs 3 seconds. Think an answer should count? Contest it at the end of the day and tonight’s judge takes a second look.',
    floorOf: 'Floor {n} of {total}', seconds: '{s}s', noTimer: 'No timer',
    readyTitle: 'Ready when you are', startFloor: 'Start floor {n}', build: 'Build', answerLabel: 'Your answer', placeholder: 'Type an answer to build a floor',
    ready: 'Floor {n}. I’m holding the block. Say when!',
    hurry: 'Hurry, my arm is getting tired!',
    'tier.1': 'Concrete', 'tier.2': 'Brick', 'tier.3': 'Glass', 'tier.4': 'Gold', 'tier.5': 'Neon',
    'line.1': 'Concrete. Solid, at least.', 'line.2': 'Nice bricks! Up we go.', 'line.3': 'Glass floor! Fancy.',
    'line.4': 'Gold-plated! The neighbours will be jealous.', 'line.5': 'NEON?! We’re lighting up the whole city!',
    rejected: '“{answer}” won’t hold. Minus {seconds} seconds!',
    timeout: 'Time’s up! The rarest answer was “{rarest}”.',
    empty: 'Type an answer first.',
    waitingTitle: 'No blueprints yet',
    waiting: 'The planner draws today’s floors overnight. Check back soon.',
    doneTitle: 'Topping out!', done: { one: 'Topping out! 1 floor, {height} m tall.', other: 'Topping out! {count} floors, {height} m tall.' },
    points: 'Points', height: 'Height', yourAnswer: 'You: {answer} · {tier} · +{points}', noFloor: 'No floor: time ran out',
    rarest: 'Rarest: {rarest}', rejectedList: 'Not accepted: {list}', contest: 'Contest “{answer}”',
    'contest.pending': 'Contested “{typed}”: tonight’s judge will decide.',
    'contest.accepted': 'Contest won: “{typed}” counts as {tier}.',
    'contest.rejected': 'Contest lost: “{typed}” does not count.',
    contested: 'Noted! Tonight’s judge takes a second look.',
    nextIn: 'Next tower in {time}', home: 'Back to QuikFlip',
    nextLandmark: 'Next: {name} ({m} m)', allLandmarks: 'Taller than the Burj Khalifa!',
    tallerThan: 'Taller than {name}!',
    'landmark.0': 'Lighthouse', 'landmark.1': 'Big Ben', 'landmark.2': 'Pyramid of Giza', 'landmark.3': 'Eiffel Tower', 'landmark.4': 'Empire State Building', 'landmark.5': 'Burj Khalifa',
    'banner.0': 'a lighthouse', 'banner.1': 'Big Ben', 'banner.2': 'the Pyramid of Giza', 'banner.3': 'the Eiffel Tower', 'banner.4': 'the Empire State Building', 'banner.5': 'the Burj Khalifa',
    timer: 'Time per prompt', 'timer.normal': '25 seconds', 'timer.relaxed': '45 seconds', 'timer.off': 'No timer',
    timerHelp: 'Applies from the next floor you start.', saveFailed: 'That setting could not be saved. Try again.',
    network: 'I can’t reach the server. Check your connection and try again.',
    jammed: 'Something jammed on the server. Try again in a moment.', retry: 'Try again',
  },
  fr: {
    back: 'Retour à QuikFlip', help: 'Comment jouer', settings: 'Réglages', close: 'Fermer',
    rules: 'Sept questions par jour. Chacune accepte beaucoup de réponses : tapes-en une avant que la grue fatigue. Chaque réponse acceptée ajoute un étage à ta tour, et les réponses rares construisent des étages plus grands et plus chics. Une mauvaise réponse coûte 3 secondes. Une réponse devrait compter ? Conteste-la en fin de journée et le juge de la nuit y jettera un œil.',
    floorOf: 'Étage {n} sur {total}', seconds: '{s} s', noTimer: 'Sans chrono',
    readyTitle: 'Prêt quand tu l’es', startFloor: 'Lancer l’étage {n}', build: 'Bâtir', answerLabel: 'Ta réponse', placeholder: 'Tape une réponse pour bâtir un étage',
    ready: 'Étage {n}. Je tiens le bloc. Dis-moi quand !',
    hurry: 'Vite, mon bras fatigue !',
    'tier.1': 'Béton', 'tier.2': 'Brique', 'tier.3': 'Verre', 'tier.4': 'Or', 'tier.5': 'Néon',
    'line.1': 'Du béton. Solide, au moins.', 'line.2': 'Belles briques ! On monte.', 'line.3': 'Un étage en verre ! Chic.',
    'line.4': 'Plaqué or ! Les voisins vont être jaloux.', 'line.5': 'DU NÉON ?! On illumine toute la ville !',
    rejected: '« {answer} » ne tiendra pas. Moins {seconds} secondes !',
    timeout: 'Temps écoulé ! La réponse la plus rare était « {rarest} ».',
    empty: 'Tape d’abord une réponse.',
    waitingTitle: 'Pas encore de plans',
    waiting: 'L’architecte dessine les étages du jour pendant la nuit. Reviens bientôt.',
    doneTitle: 'Bouquet final !', done: { one: 'Bouquet final ! 1 étage, {height} m de haut.', other: 'Bouquet final ! {count} étages, {height} m de haut.' },
    points: 'Points', height: 'Hauteur', yourAnswer: 'Toi : {answer} · {tier} · +{points}', noFloor: 'Pas d’étage : temps écoulé',
    rarest: 'La plus rare : {rarest}', rejectedList: 'Refusées : {list}', contest: 'Contester « {answer} »',
    'contest.pending': '« {typed} » contesté : le juge de la nuit décidera.',
    'contest.accepted': 'Contestation gagnée : « {typed} » compte comme {tier}.',
    'contest.rejected': 'Contestation perdue : « {typed} » ne compte pas.',
    contested: 'C’est noté ! Le juge de la nuit y jettera un œil.',
    nextIn: 'Prochaine tour dans {time}', home: 'Retour à QuikFlip',
    nextLandmark: 'Prochain : {name} ({m} m)', allLandmarks: 'Plus haut que le Burj Khalifa !',
    tallerThan: 'Plus haut que {name} !',
    'landmark.0': 'Phare', 'landmark.1': 'Big Ben', 'landmark.2': 'Pyramide de Gizeh', 'landmark.3': 'Tour Eiffel', 'landmark.4': 'Empire State Building', 'landmark.5': 'Burj Khalifa',
    'banner.0': 'un phare', 'banner.1': 'Big Ben', 'banner.2': 'la pyramide de Gizeh', 'banner.3': 'la tour Eiffel', 'banner.4': 'l’Empire State Building', 'banner.5': 'le Burj Khalifa',
    timer: 'Temps par question', 'timer.normal': '25 secondes', 'timer.relaxed': '45 secondes', 'timer.off': 'Sans chrono',
    timerHelp: 'S’applique à partir du prochain étage lancé.', saveFailed: 'Ce réglage n’a pas pu être enregistré. Réessaie.',
    network: 'Je n’arrive pas à joindre le serveur. Vérifie ta connexion et réessaie.',
    jammed: 'Quelque chose a coincé sur le serveur. Réessaie dans un instant.', retry: 'Réessayer',
  },
  nl: {
    back: 'Terug naar QuikFlip', help: 'Spelregels', settings: 'Instellingen', close: 'Sluiten',
    rules: 'Zeven vragen per dag. Elke vraag heeft veel goede antwoorden: typ er een voor de kraan moe wordt. Elk goedgekeurd antwoord voegt een verdieping toe aan je toren, en zeldzame antwoorden bouwen grotere, chiquere verdiepingen. Een fout antwoord kost 3 seconden. Vind je dat een antwoord moet tellen? Betwist het op het einde van de dag en de nachtrechter kijkt er nog eens naar.',
    floorOf: 'Verdieping {n} van {total}', seconds: '{s} s', noTimer: 'Geen timer',
    readyTitle: 'Klaar wanneer jij het bent', startFloor: 'Start verdieping {n}', build: 'Bouwen', answerLabel: 'Jouw antwoord', placeholder: 'Typ een antwoord om een verdieping te bouwen',
    ready: 'Verdieping {n}. Ik hou het blok vast. Zeg maar wanneer!',
    hurry: 'Snel, mijn arm wordt moe!',
    'tier.1': 'Beton', 'tier.2': 'Baksteen', 'tier.3': 'Glas', 'tier.4': 'Goud', 'tier.5': 'Neon',
    'line.1': 'Beton. Stevig, dat wel.', 'line.2': 'Mooie bakstenen! Naar boven.', 'line.3': 'Een glazen verdieping! Chic.',
    'line.4': 'Verguld! De buren worden jaloers.', 'line.5': 'NEON?! We verlichten de hele stad!',
    rejected: '“{answer}” houdt het niet. {seconds} seconden minder!',
    timeout: 'De tijd is op! Het zeldzaamste antwoord was “{rarest}”.',
    empty: 'Typ eerst een antwoord.',
    waitingTitle: 'Nog geen bouwplannen',
    waiting: 'De architect tekent de verdiepingen van vandaag ’s nachts. Kom straks terug.',
    doneTitle: 'Hoogste punt!', done: { one: 'Hoogste punt! 1 verdieping, {height} m hoog.', other: 'Hoogste punt! {count} verdiepingen, {height} m hoog.' },
    points: 'Punten', height: 'Hoogte', yourAnswer: 'Jij: {answer} · {tier} · +{points}', noFloor: 'Geen verdieping: de tijd was op',
    rarest: 'Zeldzaamste: {rarest}', rejectedList: 'Niet goedgekeurd: {list}', contest: '“{answer}” betwisten',
    'contest.pending': '“{typed}” betwist: de nachtrechter beslist.',
    'contest.accepted': 'Gewonnen: “{typed}” telt als {tier}.',
    'contest.rejected': 'Verloren: “{typed}” telt niet.',
    contested: 'Genoteerd! De nachtrechter kijkt er nog eens naar.',
    nextIn: 'Volgende toren over {time}', home: 'Terug naar QuikFlip',
    nextLandmark: 'Volgende: {name} ({m} m)', allLandmarks: 'Hoger dan de Burj Khalifa!',
    tallerThan: 'Hoger dan {name}!',
    'landmark.0': 'Vuurtoren', 'landmark.1': 'Big Ben', 'landmark.2': 'Piramide van Gizeh', 'landmark.3': 'Eiffeltoren', 'landmark.4': 'Empire State Building', 'landmark.5': 'Burj Khalifa',
    'banner.0': 'een vuurtoren', 'banner.1': 'de Big Ben', 'banner.2': 'de piramide van Gizeh', 'banner.3': 'de Eiffeltoren', 'banner.4': 'het Empire State Building', 'banner.5': 'de Burj Khalifa',
    timer: 'Tijd per vraag', 'timer.normal': '25 seconden', 'timer.relaxed': '45 seconden', 'timer.off': 'Geen timer',
    timerHelp: 'Geldt vanaf de volgende verdieping die je start.', saveFailed: 'Die instelling kon niet worden opgeslagen. Probeer opnieuw.',
    network: 'Ik kan de server niet bereiken. Controleer je verbinding en probeer opnieuw.',
    jammed: 'Er liep iets vast op de server. Probeer het zo meteen opnieuw.', retry: 'Opnieuw proberen',
  },
};
const TIMER_OPTIONS = ['normal', 'relaxed', 'off'];

/* ---------- Pure helpers ---------- */
const escapeHtml = text => String(text).replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]);
const floorsOf = game => game.prompts.filter(prompt => prompt.outcome === 'answered' && prompt.tier);
const towerMeters = floors => floors.reduce((sum, prompt) => sum + TIERS[prompt.tier - 1].meters, 0);
const isDark = () => matchMedia('(prefers-color-scheme: dark)').matches;
const BURJ = LANDMARKS.at(-1);

function mixHex(from, to, amount) {
  const parse = hex => [1, 3, 5].map(index => parseInt(hex.slice(index, index + 2), 16));
  const [a, b] = [parse(from), parse(to)];
  return `rgb(${a.map((value, index) => Math.round(value + (b[index] - value) * amount)).join(' ')})`;
}

function skyAt(share) {
  const stops = isDark() ? SKY_NIGHT : SKY_DAY;
  const upper = stops.findIndex(([at]) => at >= share);
  if (upper <= 0) return { top: stops[0][1], bottom: stops[0][2] };
  const [fromAt, fromTop, fromBottom] = stops[upper - 1];
  const [toAt, toTop, toBottom] = stops[upper];
  const amount = (share - fromAt) / (toAt - fromAt);
  return { top: mixHex(fromTop, toTop, amount), bottom: mixHex(fromBottom, toBottom, amount) };
}

/* ---------- DOM ---------- */
const viewport = document.getElementById('viewport');
const camera = document.getElementById('camera');
const tower = document.getElementById('tower');
const crate = document.getElementById('crate');
const heightLabel = document.getElementById('height');
const nextLabel = document.getElementById('next-landmark');
const meta = document.getElementById('meta');
const promptTitle = document.getElementById('prompt');
const speech = document.getElementById('speech');
const speechText = document.getElementById('speech-text');
const startButton = document.getElementById('start');
const form = document.getElementById('build-form');
const input = document.getElementById('answer');
const statusLine = document.getElementById('status');
const banner = document.getElementById('banner');
const pointsLabel = document.getElementById('points');
const summary = document.getElementById('summary');

let t = translator(STRINGS, 'en');
let game = null;
let settings = null;
let busy = false;
let currentMood = 'idle';
let shownMeters = 0;
let timerFrame = 0;
let countFrame = 0;
let stopCountdown = () => {};
let urgentAnnounced = false;
const timeouts = new Set();
const after = (ms, callback) => {
  const id = setTimeout(() => { timeouts.delete(id); callback(); }, ms);
  timeouts.add(id);
};
const restartClass = (element, className) => {
  element.classList.remove(className);
  void element.offsetWidth;
  element.classList.add(className);
};

function say(line, mood = currentMood) {
  speechText.textContent = line;
  restartClass(speech, 'is-new');
  viewport.classList.replace(`mood-${currentMood}`, `mood-${mood}`);
  currentMood = mood;
}

/* ---------- World ---------- */
function renderStatic() {
  document.documentElement.lang = game.language;
  camera.style.setProperty('--world-px', `${WORLD_PX}px`);
  viewport.style.setProperty('--ground-px', `${GROUND_PX}px`);
  document.getElementById('back').setAttribute('aria-label', t('back'));
  document.getElementById('help').setAttribute('aria-label', t('help'));
  document.getElementById('open-settings').setAttribute('aria-label', t('settings'));
  document.getElementById('answer-label').textContent = t('answerLabel');
  document.getElementById('build-label').textContent = t('build');
  input.placeholder = t('placeholder');
  document.getElementById('landmarks').innerHTML = LANDMARKS.map((landmark, index) => `
    <div class="landmark" data-landmark="${index}" style="--x: ${landmark.x}; --w: ${landmark.w}px; --h: ${landmark.meters * PX_PER_METER}px; --shape: ${landmark.shape}">
      <span class="landmark-shape"></span><span class="landmark-label">${t(`landmark.${index}`)} &middot; ${landmark.meters} m</span>
    </div>`).join('');
  document.getElementById('houses').innerHTML = Array.from({ length: 14 }, (_, index) => `<i style="--w: ${22 + (index * 7) % 18}px; --h: ${18 + (index * 11) % 22}px"></i>`).join('');
  document.getElementById('clouds').innerHTML = CLOUDS.map(([x, meters, width, seconds]) =>
    `<span class="cloud" style="--x: ${x}%; --y: ${meters * PX_PER_METER + GROUND_PX}px; --w: ${width}px; --d: ${seconds}s"></span>`).join('');
}

const floorHtml = (prompt, extraClass = '') => `
  <div class="floor tier-${prompt.tier} ${extraClass}" style="--h: ${TIERS[prompt.tier - 1].meters * PX_PER_METER}px; --fall: ${DROP_PX}px" title="${escapeHtml(prompt.prompt ?? '')}">
    <span class="sign">${escapeHtml(prompt.answer)}</span>
  </div>`;

function renderTower(floors) {
  tower.innerHTML = `<div class="lobby"><span></span></div>${floors.map(prompt => floorHtml(prompt)).join('')}`;
}

// Camera, crane, sky and landmarks all follow the tower's height.
function updateWorld(meters) {
  const towerPx = meters * PX_PER_METER;
  viewport.style.setProperty('--tower-px', `${towerPx}px`);
  const camOffset = Math.max(0, GROUND_PX + towerPx + 150 - viewport.clientHeight * CAMERA_TOP_SHARE);
  camera.style.setProperty('--cam', `${camOffset}px`);
  const share = Math.min(1, meters / BURJ.meters);
  const { top, bottom } = skyAt(share);
  viewport.style.setProperty('--sky-top', top);
  viewport.style.setProperty('--sky-bottom', bottom);
  viewport.style.setProperty('--star-opacity', Math.max(0, (share - 0.45) * 1.8).toFixed(2));
  document.querySelectorAll('[data-landmark]').forEach((element, index) => element.classList.toggle('is-passed', meters >= LANDMARKS[index].meters));
  const next = LANDMARKS.findIndex(landmark => landmark.meters > meters);
  nextLabel.textContent = next === -1 ? t('allLandmarks') : t('nextLandmark', { name: t(`landmark.${next}`), m: LANDMARKS[next].meters });
}

function countUpTo(target) {
  cancelAnimationFrame(countFrame);
  const from = shownMeters;
  let startTime = null;
  const step = now => {
    startTime ??= now;
    const progress = Math.min(1, (now - startTime) / COUNT_UP_MS);
    shownMeters = Math.round(from + (target - from) * (1 - (1 - progress) ** 3));
    heightLabel.textContent = `${shownMeters} m`;
    if (progress < 1) countFrame = requestAnimationFrame(step);
  };
  countFrame = requestAnimationFrame(step);
}

function setHeight(meters) {
  cancelAnimationFrame(countFrame);
  shownMeters = meters;
  heightLabel.textContent = `${meters} m`;
}

/* ---------- Timer (server deadline, counted down locally) ---------- */
function stopTimer() {
  cancelAnimationFrame(timerFrame);
  viewport.classList.remove('is-urgent');
}

function runTimer(prompt) {
  stopTimer();
  urgentAnnounced = false;
  if (prompt.remainingMs == null || !prompt.timeLimit) {
    crate.style.setProperty('--hold', 1);
    return;
  }
  const deadline = Date.now() + prompt.remainingMs;
  const step = () => {
    const left = Math.max(0, deadline - Date.now()) / 1000;
    crate.style.setProperty('--hold', (left / prompt.timeLimit).toFixed(3));
    meta.textContent = `${t('floorOf', { n: prompt.position + 1, total: game.prompts.length })} · ${t('seconds', { s: Math.ceil(left) })}`;
    const urgent = left <= URGENT_SECONDS;
    viewport.classList.toggle('is-urgent', urgent);
    if (urgent && !urgentAnnounced) {
      urgentAnnounced = true;
      say(t('hurry'), 'panic');
    }
    if (left <= 0) {
      expire(prompt.position);
      return;
    }
    timerFrame = requestAnimationFrame(step);
  };
  timerFrame = requestAnimationFrame(step);
}

/* ---------- Rendering the current step ---------- */
function render() {
  stopTimer();
  summary.hidden = true;
  startButton.hidden = true;
  form.hidden = true;
  crate.className = 'crate';
  if (game.status === 'waiting') {
    promptTitle.textContent = t('waitingTitle');
    promptTitle.classList.add('is-hidden');
    meta.textContent = '';
    say(t('waiting'), 'meh');
    return;
  }
  if (game.status === 'solved') {
    promptTitle.textContent = t('doneTitle');
    promptTitle.classList.remove('is-hidden');
    meta.textContent = '';
    renderSummary();
    return;
  }
  const prompt = game.prompts[game.current];
  const timerText = prompt.timeLimit ?? game.timer;
  meta.textContent = `${t('floorOf', { n: prompt.position + 1, total: game.prompts.length })} · ${timerText ? t('seconds', { s: timerText }) : t('noTimer')}`;
  if (prompt.outcome === 'ready') {
    promptTitle.textContent = t('readyTitle');
    promptTitle.classList.add('is-hidden');
    startButton.textContent = t('startFloor', { n: prompt.position + 1 });
    startButton.hidden = false;
    crate.style.setProperty('--hold', 1);
    say(t('ready', { n: prompt.position + 1 }), 'idle');
    return;
  }
  promptTitle.textContent = prompt.prompt;
  promptTitle.classList.remove('is-hidden');
  form.hidden = false;
  input.disabled = false;
  form.querySelector('button').disabled = false;
  input.focus();
  runTimer(prompt);
}

function tierLabel(tier) {
  return t(`tier.${tier}`);
}

function renderSummary() {
  const floors = floorsOf(game);
  const meters = towerMeters(floors);
  const items = game.prompts.map(prompt => {
    const answered = prompt.outcome === 'answered';
    const chip = answered ? `<span class="floor-chip" style="--c: ${TIERS[prompt.tier - 1].color}"></span>` : '<span class="floor-chip chip-none"></span>';
    const lines = [`<b>${escapeHtml(prompt.prompt ?? '')}</b>`];
    lines.push(answered
      ? `<span>${escapeHtml(t('yourAnswer', { answer: prompt.answer, tier: tierLabel(prompt.tier), points: prompt.points }))}</span>`
      : `<span>${t('noFloor')}</span>`);
    if (prompt.rarest) lines.push(`<span class="muted">${escapeHtml(t('rarest', { rarest: prompt.rarest }))}</span>`);
    if (prompt.contest) {
      const status = prompt.contest.status;
      lines.push(`<span>${escapeHtml(t(`contest.${status}`, { typed: prompt.contest.typed, tier: prompt.contest.tier ? tierLabel(prompt.contest.tier) : '' }))}</span>`);
    } else if (prompt.rejected.length) {
      lines.push(`<span class="muted">${escapeHtml(t('rejectedList', { list: prompt.rejected.join(', ') }))}</span>`);
      lines.push(...prompt.rejected.map(answer => `<button type="button" class="contest-btn" data-contest="${prompt.position}" data-text="${escapeHtml(answer)}">${escapeHtml(t('contest', { answer }))}</button>`));
    }
    return `<li>${chip}<span class="floor-text">${lines.join('')}</span><span class="floor-points">${prompt.points ? `+${prompt.points}` : ''}</span></li>`;
  }).join('');
  summary.innerHTML = `
    <div class="summary-card">
      <h2>${t('doneTitle')}</h2>
      <div class="summary-stats"><span>${t('points')}: <strong>${game.points}</strong></span><span>${t('height')}: <strong>${meters} m</strong></span></div>
      <ol class="floors">${items}</ol>
      <div class="summary-foot"><span class="countdown" data-countdown></span><a href="../../">${t('home')}</a></div>
    </div>`;
  summary.hidden = false;
  say(t('done', { count: floors.length, height: meters }), 'happy');
}

/* ---------- Effects ---------- */
function dustBurst() {
  for (let index = 0; index < 8; index += 1) {
    const puff = document.createElement('span');
    puff.className = 'dust';
    puff.style.setProperty('--dx', `${(index - 3.5) * 16}px`);
    tower.append(puff);
    after(750, () => puff.remove());
  }
}

function dropFloor(prompt, metersBefore) {
  crate.classList.add('is-gone');
  tower.insertAdjacentHTML('beforeend', floorHtml(prompt, 'is-dropping'));
  const meters = towerMeters(floorsOf(game));
  after(DROP_MS, () => {
    restartClass(tower, 'is-wobble');
    dustBurst();
    pointsLabel.textContent = `+${prompt.points}`;
    restartClass(pointsLabel, 'is-on');
    updateWorld(meters);
    countUpTo(meters);
    say(t(`line.${prompt.tier}`), prompt.tier >= 4 ? 'wow' : prompt.tier === 1 ? 'meh' : 'happy');
    const passed = LANDMARKS.map((landmark, index) => ({ ...landmark, index }))
      .filter(landmark => landmark.meters > metersBefore && landmark.meters <= meters).at(-1);
    if (passed) {
      banner.textContent = t('tallerThan', { name: t(`banner.${passed.index}`) });
      after(500, () => restartClass(banner, 'is-on'));
    }
  });
}

/* ---------- Actions ---------- */
function explain(error) {
  if (error.code === 'empty_answer') return t('empty');
  return error.code === 'network' ? t('network') : t('jammed');
}

async function act(request) {
  if (busy) return null;
  busy = true;
  try {
    return await request();
  } catch (error) {
    say(explain(error), 'oops');
    return null;
  } finally {
    busy = false;
  }
}

async function onStart() {
  const position = game.current;
  const next = await act(() => api.rarefindStart(position));
  if (!next) return;
  game = next;
  render();
}

async function onSubmit(event) {
  event.preventDefault();
  const text = input.value.trim();
  if (!text) {
    say(t('empty'), 'oops');
    return;
  }
  const position = game.current;
  const metersBefore = towerMeters(floorsOf(game));
  input.disabled = true;
  const next = await act(() => api.rarefindAnswer(position, text));
  input.disabled = false;
  if (!next) return;
  game = next;
  const prompt = game.prompts[position];
  if (next.result.kind === 'accepted') {
    stopTimer();
    input.value = '';
    form.hidden = true;
    dropFloor(prompt, metersBefore);
    after(NEXT_PROMPT_DELAY_MS, render);
  } else if (next.result.kind === 'rejected') {
    input.value = '';
    input.focus();
    restartClass(crate, 'is-falling');
    say(t('rejected', { answer: text, seconds: next.result.penaltySeconds }), 'oops');
    after(CRATE_RESET_MS, () => { crate.className = 'crate is-new'; });
    runTimer(prompt);
  } else {
    showTimeout(prompt);
  }
}

async function expire(position) {
  stopTimer();
  form.hidden = true;
  const next = await act(() => api.rarefindExpire(position));
  if (!next) return;
  game = next;
  const prompt = game.prompts[position];
  if (prompt.outcome === 'timedOut') showTimeout(prompt);
  else render();
}

function showTimeout(prompt) {
  stopTimer();
  form.hidden = true;
  restartClass(crate, 'is-falling');
  say(t('timeout', { rarest: prompt.rarest ?? '' }), 'oops');
  after(TIMEOUT_NEXT_DELAY_MS, render);
}

async function onContest(position, text) {
  const next = await act(() => api.rarefindContest(position, text));
  if (!next) return;
  game = next;
  renderSummary();
  say(t('contested'), 'happy');
}

/* ---------- Dialogs ---------- */
function openSettings() {
  const settingsForm = document.createElement('form');
  settingsForm.className = 'settings';
  settingsForm.innerHTML = `
    <fieldset>
      <legend>${t('timer')}</legend>
      ${TIMER_OPTIONS.map(option => `<label class="option"><input type="radio" name="timer" value="${option}" ${settings.rarefind.timer === option ? 'checked' : ''}>${t(`timer.${option}`)}</label>`).join('')}
    </fieldset>
    <p class="help">${t('timerHelp')}</p>
    <p class="settings-error" role="alert" hidden></p>`;
  const errorLine = settingsForm.querySelector('.settings-error');
  settingsForm.addEventListener('submit', event => event.preventDefault());
  settingsForm.addEventListener('change', async () => {
    const previous = settings.rarefind;
    try {
      settings = await api.saveRarefindSettings({ timer: settingsForm.elements.timer.value });
      game.timer = { normal: 25, relaxed: 45, off: null }[settings.rarefind.timer];
      errorLine.hidden = true;
    } catch {
      settingsForm.elements.timer.value = previous.timer;
      errorLine.textContent = t('saveFailed');
      errorLine.hidden = false;
    }
  });
  showDialog({ title: t('settings'), content: settingsForm, closeLabel: t('close') });
}

/* ---------- Loading ---------- */
async function load() {
  timeouts.forEach(clearTimeout);
  timeouts.clear();
  stopTimer();
  try {
    [settings, game] = await Promise.all([api.settings(), api.rarefind()]);
  } catch (error) {
    speechText.innerHTML = `${error.code === 'network' ? t('network') : t('jammed')} <button type="button" data-retry>${t('retry')}</button>`;
    return;
  }
  t = translator(STRINGS, game.language);
  renderStatic();
  const floors = floorsOf(game);
  renderTower(floors);
  setHeight(towerMeters(floors));
  updateWorld(towerMeters(floors));
  render();
  stopCountdown();
  stopCountdown = startCountdown(game.secondsToReset, left => {
    const element = summary.querySelector('[data-countdown]');
    if (element) element.textContent = t('nextIn', { time: formatDuration(left) });
  }, () => location.reload());
}

startButton.addEventListener('click', onStart);
form.addEventListener('submit', onSubmit);
summary.addEventListener('click', event => {
  const button = event.target.closest('[data-contest]');
  if (button) onContest(Number(button.dataset.contest), button.dataset.text);
});
speech.addEventListener('click', event => {
  if (event.target.closest('[data-retry]')) load();
});
document.getElementById('help').addEventListener('click', () => showDialog({ title: t('help'), content: `<p>${t('rules')}</p>`, closeLabel: t('close') }));
document.getElementById('open-settings').addEventListener('click', () => { if (settings) openSettings(); });
window.addEventListener('resize', () => { if (game) updateWorld(towerMeters(floorsOf(game))); });

load();
