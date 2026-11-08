// Atlas Hunt "Tiny Planet": pick countries on a three.js planet. Found countries rise as pillars (rarer = taller) and
// launch a moon into orbit. The server owns the puzzle; this page only draws it and sends picks.
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { TessellateModifier } from 'three/addons/modifiers/TessellateModifier.js';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/addons/postprocessing/OutputPass.js';
import { api } from '../../js/api.js';
import { translator } from '../../js/i18n.js';
import { showDialog } from '../../js/dialog.js';
import { formatDuration, startCountdown } from '../../js/countdown.js';

/* ---------- Tuning ---------- */
// Rarer finds rise higher out of the planet and launch bigger moons onto wider, slower orbits.
const TIERS = [
  { height: 0.035, moon: 0.045, orbit: 1.45, speed: 0.55, color: '#cfd8e6' },
  { height: 0.06, moon: 0.06, orbit: 1.65, speed: 0.45, color: '#5be3b5' },
  { height: 0.095, moon: 0.075, orbit: 1.9, speed: 0.36, color: '#ffd166' },
  { height: 0.14, moon: 0.095, orbit: 2.2, speed: 0.28, color: '#ff8a5b' },
  { height: 0.22, moon: 0.12, orbit: 2.55, speed: 0.2, color: '#b48cff' },
];
const WRONG_COLOR = '#ff4d5e';
const R = 1;
const DEG = Math.PI / 180;
const CAMERA_DISTANCE = 3.6;
const FLY_MS = 1000;
const RISE_MS = 1300;
const LAUNCH_DELAY_MS = 550;
const LAUNCH_MS = 1300;
const MISS_MS = 900;
const SHAKE_MS = 600;
const NEXT_CLUE_MS = 2200;
const LOST_NEXT_MS = 3200;
const DENSIFY_DEG = 1.5;
const TESSELLATE_DEG = 2;
const CLICK_SLOP_PX = 6;
const CLOUD_COUNT = 16;
const STAR_COUNT = 1600;
const ZOOM_STEP = 1.25;
const MIN_DISTANCE = 1.7;
const MAX_DISTANCE = 6.5;
// Countries this small (in steradians) or without a map shape get a hexagonal column instead of a raised outline.
const SMALL_AREA_SR = 0.00012;
const COLUMN_RADIUS = 0.012;
const SMALL_PICK_DEG = 1.6;
const TEXTURE_WIDTH = 4096;
const OUTSIDE_REGION = '#9aa4ab';
// Where the camera looks first for each region: [latitude, longitude].
const REGION_VIEW = { world: [15, 10], europe: [50, 15], africa: [3, 20], americas: [12, -78], asia: [32, 92], oceania: [-18, 150] };
const REGIONS = Object.keys(REGION_VIEW);
const MUTE_KEY = 'quikflip.atlas.muted';

const STRINGS = {
  en: {
    back: 'Back to QuikFlip', help: 'How to play', settings: 'Settings', close: 'Close', mute: 'Mute sound', unmute: 'Unmute sound',
    zoomIn: 'Zoom in', zoomOut: 'Zoom out', zoomReset: 'Reset view', points: 'Points',
    rules: 'Five clues a day. Each clue fits several countries: spin the planet and click one, or type its name. Every country that fits scores, and rarer picks score more: they rise higher and launch bigger moons. A country can only be used once a day. Three misses and the clue is lost. Every find comes with a fun fact.',
    clueOf: 'Clue {n} of {total}', fits: { one: '1 country fits', other: '{count} countries fit' },
    inputLabel: 'Type a country', placeholder: 'Click the planet, or type a country', raise: 'RAISE',
    controls: 'Drag to spin the planet. Scroll, pinch or use + and − to zoom.',
    start: 'Pick a country that fits the clue and watch it rise!',
    flying: 'Flying to {name}…', rising: '{name} is rising…',
    'tier.1': 'Common', 'tier.2': 'Uncommon', 'tier.3': 'Rare', 'tier.4': 'Very rare', 'tier.5': 'Legendary',
    'banner.1': 'COMMON', 'banner.2': 'UNCOMMON!', 'banner.3': 'RARE!', 'banner.4': 'VERY RARE!', 'banner.5': 'LEGENDARY!!',
    'line.1': 'A common pick. Up it comes!', 'line.2': 'Uncommon! Nice one.', 'line.3': 'Rare! Not many people pick that one.',
    'line.4': 'Very rare! Look at that tower.', 'line.5': 'LEGENDARY! Almost nobody finds that one.',
    plusPoints: '+{points} points',
    miss: { one: 'Bonk! {name} doesn’t fit. 1 try left.', other: 'Bonk! {name} doesn’t fit. {count} tries left.' },
    lost: 'Bonk! Out of tries for this clue. Rare ones were: {rarest}.',
    alreadyTried: 'You already tried {name} today. Pick another.',
    outsideRegion: '{name} is outside today’s region ({region}).',
    notACountry: 'That is not one of the countries in this game.',
    unknownName: 'I can’t find “{text}” on this planet.',
    factLabel: 'Did you know?', nextClue: 'Next clue', seeResults: 'See results',
    found: 'Found: {name}', lostChip: 'Clue {n} lost', emptyChip: 'Clue {n}',
    waiting: 'Today’s planet is still being mapped. Check back soon.',
    doneTitle: 'Planet complete!', done: 'All five clues done. Come back tomorrow for a new planet.',
    total: 'Points', moons: 'Moons', yourPick: 'You: {name} · {tier} · +{points}', noPick: 'Lost after three misses',
    rarestList: 'Rarest: {list}', missed: 'Misses: {list}', nextIn: 'New clues in {time}', home: 'Back to QuikFlip', viewPlanet: 'View planet',
    region: 'Region', 'region.world': 'Whole world', 'region.europe': 'Europe', 'region.africa': 'Africa', 'region.americas': 'Americas', 'region.asia': 'Asia', 'region.oceania': 'Oceania',
    regionHelp: 'A new region applies from tomorrow’s clues.', regionNext: 'From tomorrow: {region}.',
    facts: 'Fun facts', factsOn: 'Show a fun fact for every country I find',
    saveFailed: 'That setting could not be saved. Try again.',
    network: 'I can’t reach the server. Check your connection and try again.',
    jammed: 'Something went wrong on the server. Try again in a moment.', retry: 'Try again',
    noWebgl: 'Atlas Hunt needs WebGL, and this browser or device has it turned off.',
  },
  fr: {
    back: 'Retour à QuikFlip', help: 'Comment jouer', settings: 'Réglages', close: 'Fermer', mute: 'Couper le son', unmute: 'Remettre le son',
    zoomIn: 'Zoomer', zoomOut: 'Dézoomer', zoomReset: 'Recentrer', points: 'Points',
    rules: 'Cinq indices par jour. Chaque indice convient à plusieurs pays : fais tourner la planète et clique sur l’un d’eux, ou tape son nom. Chaque pays qui convient rapporte des points, et les choix rares en rapportent plus : ils montent plus haut et lancent de plus grosses lunes. Un pays ne sert qu’une fois par jour. Trois ratés et l’indice est perdu. Chaque trouvaille vient avec une anecdote.',
    clueOf: 'Indice {n} sur {total}', fits: { one: '1 pays convient', other: '{count} pays conviennent' },
    inputLabel: 'Tape un pays', placeholder: 'Clique sur la planète ou tape un pays', raise: 'LEVER',
    controls: 'Fais glisser pour tourner la planète. Molette, pincement ou + et − pour zoomer.',
    start: 'Choisis un pays qui correspond à l’indice et regarde-le monter !',
    flying: 'En route vers : {name}…', rising: '{name} sort de terre…',
    'tier.1': 'Courant', 'tier.2': 'Peu courant', 'tier.3': 'Rare', 'tier.4': 'Très rare', 'tier.5': 'Légendaire',
    'banner.1': 'COURANT', 'banner.2': 'PEU COURANT !', 'banner.3': 'RARE !', 'banner.4': 'TRÈS RARE !', 'banner.5': 'LÉGENDAIRE !!',
    'line.1': 'Un choix courant. Ça monte !', 'line.2': 'Peu courant ! Bien joué.', 'line.3': 'Rare ! Peu de gens y pensent.',
    'line.4': 'Très rare ! Regarde-moi cette tour.', 'line.5': 'LÉGENDAIRE ! Presque personne ne trouve celui-là.',
    plusPoints: '+{points} points',
    miss: { one: 'Bonk ! {name} ne convient pas. Encore 1 essai.', other: 'Bonk ! {name} ne convient pas. Encore {count} essais.' },
    lost: 'Bonk ! Plus d’essais pour cet indice. Les plus rares étaient : {rarest}.',
    alreadyTried: 'Tu as déjà essayé {name} aujourd’hui. Choisis-en un autre.',
    outsideRegion: '{name} est hors de la région du jour ({region}).',
    notACountry: 'Ce n’est pas un des pays de ce jeu.',
    unknownName: 'Je ne trouve pas « {text} » sur cette planète.',
    factLabel: 'Le savais-tu ?', nextClue: 'Indice suivant', seeResults: 'Voir le résultat',
    found: 'Trouvé : {name}', lostChip: 'Indice {n} perdu', emptyChip: 'Indice {n}',
    waiting: 'La planète du jour est encore en cartographie. Reviens bientôt.',
    doneTitle: 'Planète terminée !', done: 'Les cinq indices sont joués. Reviens demain pour une nouvelle planète.',
    total: 'Points', moons: 'Lunes', yourPick: 'Toi : {name} · {tier} · +{points}', noPick: 'Perdu après trois ratés',
    rarestList: 'Les plus rares : {list}', missed: 'Ratés : {list}', nextIn: 'Nouveaux indices dans {time}', home: 'Retour à QuikFlip', viewPlanet: 'Voir la planète',
    region: 'Région', 'region.world': 'Le monde entier', 'region.europe': 'Europe', 'region.africa': 'Afrique', 'region.americas': 'Amériques', 'region.asia': 'Asie', 'region.oceania': 'Océanie',
    regionHelp: 'Une nouvelle région s’applique à partir des indices de demain.', regionNext: 'À partir de demain : {region}.',
    facts: 'Anecdotes', factsOn: 'Montrer une anecdote pour chaque pays trouvé',
    saveFailed: 'Ce réglage n’a pas pu être enregistré. Réessaie.',
    network: 'Je n’arrive pas à joindre le serveur. Vérifie ta connexion et réessaie.',
    jammed: 'Un problème est survenu sur le serveur. Réessaie dans un instant.', retry: 'Réessayer',
    noWebgl: 'Atlas Hunt a besoin de WebGL, et ce navigateur ou cet appareil l’a désactivé.',
  },
  nl: {
    back: 'Terug naar QuikFlip', help: 'Spelregels', settings: 'Instellingen', close: 'Sluiten', mute: 'Geluid uit', unmute: 'Geluid aan',
    zoomIn: 'Inzoomen', zoomOut: 'Uitzoomen', zoomReset: 'Beeld herstellen', points: 'Punten',
    rules: 'Vijf hints per dag. Elke hint past bij meerdere landen: draai de planeet en klik er een aan, of typ de naam. Elk land dat past levert punten op, en zeldzame keuzes meer: ze rijzen hoger en lanceren grotere manen. Een land kan maar één keer per dag gebruikt worden. Drie missers en de hint is verloren. Bij elke vondst hoort een leuk weetje.',
    clueOf: 'Hint {n} van {total}', fits: { one: '1 land past', other: '{count} landen passen' },
    inputLabel: 'Typ een land', placeholder: 'Klik op de planeet of typ een land', raise: 'OMHOOG',
    controls: 'Sleep om de planeet te draaien. Scroll, knijp of gebruik + en − om te zoomen.',
    start: 'Kies een land dat bij de hint past en kijk hoe het omhoog komt!',
    flying: 'Op weg naar {name}…', rising: '{name} komt omhoog…',
    'tier.1': 'Gewoon', 'tier.2': 'Ongewoon', 'tier.3': 'Zeldzaam', 'tier.4': 'Heel zeldzaam', 'tier.5': 'Legendarisch',
    'banner.1': 'GEWOON', 'banner.2': 'ONGEWOON!', 'banner.3': 'ZELDZAAM!', 'banner.4': 'HEEL ZELDZAAM!', 'banner.5': 'LEGENDARISCH!!',
    'line.1': 'Een gewone keuze. Daar komt hij!', 'line.2': 'Ongewoon! Goed gedaan.', 'line.3': 'Zeldzaam! Daar denken weinig mensen aan.',
    'line.4': 'Heel zeldzaam! Moet je die toren zien.', 'line.5': 'LEGENDARISCH! Bijna niemand vindt die.',
    plusPoints: '+{points} punten',
    miss: { one: 'Bonk! {name} past niet. Nog 1 poging.', other: 'Bonk! {name} past niet. Nog {count} pogingen.' },
    lost: 'Bonk! Geen pogingen meer voor deze hint. Zeldzame antwoorden waren: {rarest}.',
    alreadyTried: 'Je probeerde {name} vandaag al. Kies een ander land.',
    outsideRegion: '{name} ligt buiten de regio van vandaag ({region}).',
    notACountry: 'Dat is geen van de landen in dit spel.',
    unknownName: 'Ik vind “{text}” niet op deze planeet.',
    factLabel: 'Wist je dat?', nextClue: 'Volgende hint', seeResults: 'Bekijk het resultaat',
    found: 'Gevonden: {name}', lostChip: 'Hint {n} verloren', emptyChip: 'Hint {n}',
    waiting: 'De planeet van vandaag wordt nog in kaart gebracht. Kom straks terug.',
    doneTitle: 'Planeet klaar!', done: 'Alle vijf hints gespeeld. Kom morgen terug voor een nieuwe planeet.',
    total: 'Punten', moons: 'Manen', yourPick: 'Jij: {name} · {tier} · +{points}', noPick: 'Verloren na drie missers',
    rarestList: 'Zeldzaamste: {list}', missed: 'Missers: {list}', nextIn: 'Nieuwe hints over {time}', home: 'Terug naar QuikFlip', viewPlanet: 'Bekijk de planeet',
    region: 'Regio', 'region.world': 'De hele wereld', 'region.europe': 'Europa', 'region.africa': 'Afrika', 'region.americas': 'Amerika', 'region.asia': 'Azië', 'region.oceania': 'Oceanië',
    regionHelp: 'Een nieuwe regio geldt vanaf de hints van morgen.', regionNext: 'Vanaf morgen: {region}.',
    facts: 'Weetjes', factsOn: 'Toon een weetje bij elk land dat ik vind',
    saveFailed: 'Die instelling kon niet worden opgeslagen. Probeer opnieuw.',
    network: 'Ik kan de server niet bereiken. Controleer je verbinding en probeer opnieuw.',
    jammed: 'Er ging iets mis op de server. Probeer het zo meteen opnieuw.', retry: 'Opnieuw proberen',
    noWebgl: 'Atlas Hunt heeft WebGL nodig, en deze browser of dit toestel heeft het uitgeschakeld.',
  },
};

/* ---------- Pure helpers ---------- */
const ARTICLES = { en: ['the'], fr: ['les', 'le', 'la', 'l'], nl: ['de', 'het'] };
const fold = text => text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();
const escapeHtml = text => String(text).replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]);
const restartClass = (element, className) => { element.classList.remove(className); void element.offsetWidth; element.classList.add(className); };
const clamp = (value, min, max) => Math.min(max, Math.max(min, value));
const easeInOut = t => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
const easeOut = t => 1 - (1 - t) ** 3;
const easeOutElastic = t => (t === 0 || t === 1 ? t : 2 ** (-10 * t) * Math.sin((t * 10 - 0.75) * ((2 * Math.PI) / 3)) + 1);
const isDark = () => matchMedia('(prefers-color-scheme: dark)').matches;
const storage = {
  get(key) { try { return localStorage.getItem(key); } catch { return null; } },
  set(key, value) { try { localStorage.setItem(key, value); } catch { /* private mode: the setting just isn't remembered */ } },
};

function normalizeName(text, lang) {
  const words = fold(text).split(' ').filter(Boolean);
  const article = (ARTICLES[lang] ?? []).find(candidate => words.length > 1 && words[0] === candidate);
  return (article ? words.slice(1) : words).join(' ');
}

// Same convention as THREE.SphereGeometry's UVs, so the equirectangular texture lines up.
function lonLatToVec(lon, lat, radius, target = new THREE.Vector3()) {
  const phi = (lon + 180) * DEG;
  const theta = (90 - lat) * DEG;
  return target.set(-radius * Math.cos(phi) * Math.sin(theta), radius * Math.cos(theta), radius * Math.sin(phi) * Math.sin(theta));
}
function vecToLonLat(vector) {
  const n = vector.clone().normalize();
  const lat = 90 - Math.acos(clamp(n.y, -1, 1)) / DEG;
  let lon = Math.atan2(n.z, -n.x) / DEG - 180;
  if (lon < -180) lon += 360;
  return [lon, lat];
}

/* ---------- DOM ---------- */
const stage = document.getElementById('stage');
const labels = document.getElementById('labels');
const clueTitle = document.getElementById('clue');
const meta = document.getElementById('meta');
const hint = document.getElementById('hint');
const speech = document.getElementById('speech');
const scoreLabel = document.getElementById('score');
const scoreCard = document.getElementById('score-card');
const findsList = document.getElementById('finds');
const banner = document.getElementById('banner');
const form = document.getElementById('launch-form');
const input = document.getElementById('country-input');
const factCard = document.getElementById('fact');
const summary = document.getElementById('summary');
const muteButton = document.getElementById('mute');

let t = translator(STRINGS, 'en');
let lang = 'en';
let game = null;
let settings = null;
let busy = false;

function say(line) {
  speech.textContent = line;
  restartClass(speech, 'is-new');
}

function showLoadError(error) {
  speech.innerHTML = `${escapeHtml(error?.code === 'network' ? t('network') : t('jammed'))} <button type="button" data-retry>${escapeHtml(t('retry'))}</button>`;
}
speech.addEventListener('click', event => {
  if (event.target.closest('[data-retry]')) location.reload();
  if (event.target.closest('[data-results]')) summary.hidden = false;
});

/* ---------- Data: the country list (shared with the server) and the map ---------- */
async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: ${response.status}`);
  return response.json();
}

let countryList;
let world;
try {
  [countryList, world, settings, game] = await Promise.all([fetchJson('data/countries.json'), fetchJson('data/world.json'), api.settings(), api.atlas()]);
} catch (error) {
  showLoadError(error);
  throw error;
}
lang = game.language;
t = translator(STRINGS, lang);
document.documentElement.lang = lang;

const byCode = new Map(countryList.map(country => [country.code, country]));
const features = topojson.feature(world, world.objects.countries).features;
const featureOf = new Map();
for (const country of countryList) {
  if (!country.shape) continue;
  const feature = features.find(candidate => candidate.id === country.shape || (!candidate.id && candidate.properties.name === country.shape));
  if (feature) featureOf.set(country.code, feature);
}
const codeOfFeature = new Map([...featureOf].map(([code, feature]) => [feature, code]));
const boundsOf = new Map([...featureOf].map(([code, feature]) => [code, d3.geoBounds(feature)]));
// Tiny countries (and those without a shape) are picked by distance to their centre and drawn as columns.
const smallCountries = countryList.filter(country => !featureOf.has(country.code) || d3.geoArea(featureOf.get(country.code)) < SMALL_AREA_SR);
const isSmall = new Set(smallCountries.map(country => country.code));
const nameOf = code => byCode.get(code)?.names[lang] ?? code;
const regionOk = code => game.region === 'world' || byCode.get(code)?.region.toLowerCase() === game.region;
const surfacePoint = (code, radius) => {
  const [lat, lon] = byCode.get(code).latlng;
  return lonLatToVec(lon, lat, radius);
};

/* ---------- Sound: tiny synthesized effects ---------- */
const sound = {
  context: null,
  muted: storage.get(MUTE_KEY) === '1',
  ensure() {
    if (!this.context) {
      const AudioContextClass = window.AudioContext || window.webkitAudioContext;
      if (!AudioContextClass) return null;
      this.context = new AudioContextClass();
    }
    this.context.resume();
    return this.context;
  },
  tone({ delay = 0, duration, from, to, type = 'sine', volume = 0.18 }) {
    if (this.muted) return;
    const context = this.ensure();
    if (!context) return;
    const start = context.currentTime + delay;
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    oscillator.type = type;
    oscillator.frequency.setValueAtTime(from, start);
    oscillator.frequency.exponentialRampToValueAtTime(to, start + duration);
    gain.gain.setValueAtTime(0.0001, start);
    gain.gain.exponentialRampToValueAtTime(volume, start + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, start + duration);
    oscillator.connect(gain).connect(context.destination);
    oscillator.start(start);
    oscillator.stop(start + duration + 0.02);
  },
  tap() { this.tone({ duration: 0.07, from: 900, to: 650, volume: 0.1 }); },
  rise(tier) { this.tone({ duration: 0.5 + tier * 0.12, from: 120, to: 300 + tier * 140, type: 'triangle', volume: 0.14 }); },
  whoosh() { this.tone({ duration: 0.9, from: 220, to: 1600, type: 'sawtooth', volume: 0.04 }); },
  pop(tier) {
    [0, 4, 7, 12].slice(0, 2 + Math.ceil(tier / 2)).forEach((step, index) => this.tone({ delay: index * 0.07, duration: 0.25, from: 523 * 2 ** (step / 12), to: 523 * 2 ** (step / 12) * 1.01, volume: 0.12 }));
  },
  bonk() { this.tone({ duration: 0.32, from: 200, to: 70, type: 'square', volume: 0.1 }); },
};

/* ---------- Renderer, camera, controls ---------- */
let renderer;
try {
  renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: 'high-performance' });
} catch (error) {
  stage.innerHTML = `<p class="fallback">${escapeHtml(t('noWebgl'))}</p>`;
  throw error;
}
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
renderer.setSize(innerWidth, innerHeight);
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.05;
stage.append(renderer.domElement);

const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(36, innerWidth / innerHeight, 0.05, 200);
const cameraHome = () => {
  const [lat, lon] = REGION_VIEW[game.region] ?? REGION_VIEW.world;
  return lonLatToVec(lon, lat, CAMERA_DISTANCE);
};
camera.position.copy(cameraHome());
scene.add(camera);
const controls = new OrbitControls(camera, renderer.domElement);
Object.assign(controls, { enableDamping: true, dampingFactor: 0.08, enablePan: false, rotateSpeed: 0.55, zoomSpeed: 0.7, minDistance: MIN_DISTANCE, maxDistance: MAX_DISTANCE });

const composer = new EffectComposer(renderer);
composer.addPass(new RenderPass(scene, camera));
const bloom = new UnrealBloomPass(new THREE.Vector2(innerWidth, innerHeight), 0.7, 0.55, 0.8);
composer.addPass(bloom);
composer.addPass(new OutputPass());

// Everything that shakes or wobbles together lives in "world".
const worldGroup = new THREE.Group();
const planet = new THREE.Group();
scene.add(worldGroup);
worldGroup.add(planet);

/* ---------- Lights: the sun rides with the camera so the side you look at is always lit ---------- */
const hemi = new THREE.HemisphereLight('#cfe9ff', '#3b2a4a', 0.9);
scene.add(hemi);
const sun = new THREE.DirectionalLight('#fff1dc', 2.6);
sun.position.set(2.5, 2.2, 3);
camera.add(sun);
const rim = new THREE.DirectionalLight('#7cc8ff', 1.4);
rim.position.set(-3, -1, -4);
camera.add(rim);

/* ---------- Background: a painted gradient; at night it fades to pure black ---------- */
function paintBackground(dark) {
  const canvas = document.createElement('canvas');
  canvas.width = 1024;
  canvas.height = 1024;
  const ctx = canvas.getContext('2d');
  const base = ctx.createRadialGradient(700, 260, 40, 560, 520, 900);
  if (dark) { base.addColorStop(0, '#12294a'); base.addColorStop(0.45, '#040a14'); base.addColorStop(1, '#000000'); }
  else { base.addColorStop(0, '#fffaf0'); base.addColorStop(0.5, '#dbe8ef'); base.addColorStop(1, '#a8c1cf'); }
  ctx.fillStyle = base;
  ctx.fillRect(0, 0, 1024, 1024);
  const warm = ctx.createRadialGradient(160, 900, 10, 160, 900, 620);
  warm.addColorStop(0, dark ? 'rgb(255 140 90 / 0.1)' : 'rgb(255 180 140 / 0.35)');
  warm.addColorStop(1, 'rgb(255 140 90 / 0)');
  ctx.fillStyle = warm;
  ctx.fillRect(0, 0, 1024, 1024);
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}

/* ---------- Ocean: animated shader with sun glints and a glowing rim ---------- */
const oceanUniforms = {
  uTime: { value: 0 },
  uSun: { value: new THREE.Vector3(1, 1, 1).normalize() },
  uDeep: { value: new THREE.Color('#0d4a72') },
  uShallow: { value: new THREE.Color('#38b6c9') },
  uRim: { value: new THREE.Color('#8fe3ff') },
};
const ocean = new THREE.Mesh(
  new THREE.SphereGeometry(R, 128, 96),
  new THREE.ShaderMaterial({
    uniforms: oceanUniforms,
    vertexShader: `
      varying vec3 vNormalW; varying vec3 vPosW; varying vec3 vPosL;
      void main() {
        vPosL = position;
        vec4 world = modelMatrix * vec4(position, 1.0);
        vPosW = world.xyz;
        vNormalW = normalize(mat3(modelMatrix) * normal);
        gl_Position = projectionMatrix * viewMatrix * world;
      }`,
    fragmentShader: `
      uniform float uTime; uniform vec3 uSun; uniform vec3 uDeep; uniform vec3 uShallow; uniform vec3 uRim;
      varying vec3 vNormalW; varying vec3 vPosW; varying vec3 vPosL;
      void main() {
        vec3 n = normalize(vNormalW);
        vec3 view = normalize(cameraPosition - vPosW);
        float facing = max(dot(n, view), 0.0);
        float fres = pow(1.0 - facing, 2.6);
        float waves = sin(vPosL.x * 38.0 + uTime * 0.9) * sin(vPosL.y * 34.0 - uTime * 0.7) * sin(vPosL.z * 41.0 + uTime * 0.8);
        vec3 col = mix(uDeep, uShallow, 0.25 + 0.12 * waves + 0.35 * facing);
        float diffuse = max(dot(n, uSun), 0.0);
        col *= 0.35 + 0.85 * diffuse;
        float glint = pow(max(dot(reflect(-uSun, n), view), 0.0), 70.0);
        col += glint * vec3(1.0, 0.95, 0.85) * (0.8 + 0.4 * waves);
        col += fres * uRim * 0.7;
        gl_FragColor = vec4(col, 1.0);
      }`,
  }),
);
planet.add(ocean);

/* ---------- Land: painted onto a texture (snow, forest, meadow, desert, jungle by latitude) ---------- */
const BIOMES = [[90, '#f5f8f9'], [68, '#e2eaec'], [60, '#4c8a68'], [48, '#76bd78'], [32, '#d9c17b'], [18, '#a8c56c'], [0, '#3fa56c'], [-18, '#a8c56c'], [-32, '#d9c17b'], [-48, '#76bd78'], [-60, '#4c8a68'], [-68, '#e2eaec'], [-90, '#f5f8f9']];
function paintLand() {
  const width = TEXTURE_WIDTH;
  const height = TEXTURE_WIDTH / 2;
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  const projection = d3.geoEquirectangular().scale(width / (2 * Math.PI)).translate([width / 2, height / 2]);
  const path = d3.geoPath(projection, ctx);
  const all = { type: 'FeatureCollection', features };
  ctx.lineJoin = 'round';
  // Sandy beaches: a wide stroke whose inner half is later covered by the land fill.
  ctx.beginPath(); path(all);
  ctx.strokeStyle = '#f2dfae'; ctx.lineWidth = 9; ctx.stroke();
  ctx.save();
  ctx.beginPath(); path(all); ctx.clip();
  const gradient = ctx.createLinearGradient(0, 0, 0, height);
  BIOMES.forEach(([lat, color]) => gradient.addColorStop((90 - lat) / 180, color));
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, width, height);
  // Countries outside today's region are greyed out, so the playable area stands out.
  if (game.region !== 'world') {
    ctx.fillStyle = OUTSIDE_REGION;
    for (const [code, feature] of featureOf) {
      if (regionOk(code)) continue;
      ctx.beginPath(); path(feature); ctx.fill();
    }
  }
  // Speckles give the land a hand-painted grain.
  for (let index = 0; index < 9000; index += 1) {
    ctx.fillStyle = index % 2 ? 'rgb(255 255 255 / 0.07)' : 'rgb(0 40 20 / 0.08)';
    ctx.beginPath();
    ctx.arc(Math.random() * width, Math.random() * height, 1 + Math.random() * 3.5, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.restore();
  ctx.beginPath(); path(all);
  ctx.strokeStyle = 'rgb(20 40 35 / 0.32)'; ctx.lineWidth = 1.6; ctx.stroke();
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.anisotropy = renderer.capabilities.getMaxAnisotropy();
  return texture;
}
const land = new THREE.Mesh(
  new THREE.SphereGeometry(R * 1.002, 160, 120),
  new THREE.MeshStandardMaterial({ map: paintLand(), transparent: true, alphaTest: 0.4, roughness: 0.92, metalness: 0 }),
);
planet.add(land);

/* ---------- Atmosphere: a soft glowing shell ---------- */
const atmosphereUniforms = { uColor: { value: new THREE.Color('#7fd6ff') }, uStrength: { value: 1 } };
worldGroup.add(new THREE.Mesh(
  new THREE.SphereGeometry(R * 1.16, 64, 48),
  new THREE.ShaderMaterial({
    uniforms: atmosphereUniforms,
    vertexShader: 'varying vec3 vNormalV; void main() { vNormalV = normalize(normalMatrix * normal); gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }',
    fragmentShader: 'uniform vec3 uColor; uniform float uStrength; varying vec3 vNormalV; void main() { float glow = pow(max(0.68 - dot(vNormalV, vec3(0.0, 0.0, 1.0)), 0.0), 3.0) * 2.4 * uStrength; gl_FragColor = vec4(uColor, 1.0) * glow; }',
    side: THREE.BackSide, blending: THREE.AdditiveBlending, transparent: true, depthWrite: false,
  }),
));

/* ---------- Clouds: puffy low-poly clusters drifting around the planet ---------- */
const cloudLayer = new THREE.Group();
planet.add(cloudLayer);
const cloudMaterial = new THREE.MeshStandardMaterial({ color: '#ffffff', roughness: 1, flatShading: true, transparent: true, opacity: 0.92 });
for (let index = 0; index < CLOUD_COUNT; index += 1) {
  const cloud = new THREE.Group();
  const puffs = 3 + Math.floor(Math.random() * 3);
  for (let puff = 0; puff < puffs; puff += 1) {
    const size = 0.022 + Math.random() * 0.03;
    const mesh = new THREE.Mesh(new THREE.IcosahedronGeometry(size, 1), cloudMaterial);
    mesh.position.set((puff - puffs / 2) * size * 1.3, Math.random() * size * 0.6, Math.random() * size * 0.5);
    mesh.scale.y = 0.75;
    cloud.add(mesh);
  }
  const normal = lonLatToVec(Math.random() * 360 - 180, (Math.random() * 2 - 1) * 62, 1);
  cloud.position.copy(normal).multiplyScalar(R * (1.06 + Math.random() * 0.05));
  cloud.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), normal);
  cloud.rotateZ(Math.random() * Math.PI);
  cloudLayer.add(cloud);
}

/* ---------- Stars ---------- */
const starPositions = new Float32Array(STAR_COUNT * 3);
for (let index = 0; index < STAR_COUNT; index += 1) {
  const direction = new THREE.Vector3(Math.random() * 2 - 1, Math.random() * 2 - 1, Math.random() * 2 - 1).normalize().multiplyScalar(40 + Math.random() * 30);
  starPositions.set([direction.x, direction.y, direction.z], index * 3);
}
const starGeometry = new THREE.BufferGeometry();
starGeometry.setAttribute('position', new THREE.BufferAttribute(starPositions, 3));
const stars = new THREE.Points(starGeometry, new THREE.PointsMaterial({ color: '#ffffff', size: 0.12, sizeAttenuation: true, transparent: true, opacity: 0.85, depthWrite: false }));
scene.add(stars);

function applyTheme() {
  const dark = isDark();
  scene.background?.dispose();
  scene.background = paintBackground(dark);
  stars.visible = dark;
  bloom.strength = dark ? 0.75 : 0.35;
  hemi.color.set(dark ? '#9fc8ff' : '#e8f4ff');
  hemi.groundColor.set(dark ? '#2a1e3a' : '#b9a58f');
  atmosphereUniforms.uStrength.value = dark ? 1 : 0.7;
}
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);
applyTheme();

/* ---------- Pillars: a country's outline tessellated onto the sphere, or a column for tiny countries ---------- */
const tessellator = new TessellateModifier(TESSELLATE_DEG, 8);
const slabCache = new Map();

function densify(ring) {
  const points = [];
  for (let index = 0; index < ring.length - 1; index += 1) {
    const [lonA, latA] = ring[index];
    const [lonB, latB] = ring[index + 1];
    const steps = Math.max(1, Math.ceil(Math.hypot(lonB - lonA, latB - latA) / DENSIFY_DEG));
    for (let step = 0; step < steps; step += 1) points.push([lonA + ((lonB - lonA) * step) / steps, latA + ((latB - latA) * step) / steps]);
  }
  return points;
}
const crossesDateLine = ring => ring.some((point, index) => index > 0 && Math.abs(point[0] - ring[index - 1][0]) > 180);

// Unit directions for every vertex plus a flag for "top cap" or "bottom edge"; the height is applied later.
function slabData(code) {
  if (slabCache.has(code)) return slabCache.get(code);
  const directions = [];
  const isTop = [];
  const push = (lon, lat, top) => {
    const v = lonLatToVec(lon, lat, 1);
    directions.push(v.x, v.y, v.z);
    isTop.push(top ? 1 : 0);
  };
  const { geometry } = featureOf.get(code);
  const polygons = geometry.type === 'Polygon' ? [geometry.coordinates] : geometry.coordinates;
  for (const polygon of polygons) {
    if (crossesDateLine(polygon[0])) continue;
    const rings = polygon.map(densify);
    const shape = new THREE.Shape(rings[0].map(([lon, lat]) => new THREE.Vector2(lon, lat)));
    rings.slice(1).forEach(hole => shape.holes.push(new THREE.Path(hole.map(([lon, lat]) => new THREE.Vector2(lon, lat)))));
    const cap = tessellator.modify(new THREE.ShapeGeometry(shape));
    const positions = cap.getAttribute('position');
    for (let index = 0; index < positions.count; index += 1) push(positions.getX(index), positions.getY(index), true);
    cap.dispose();
    for (const ring of rings) {
      for (let index = 0; index < ring.length; index += 1) {
        const a = ring[index];
        const b = ring[(index + 1) % ring.length];
        push(a[0], a[1], false); push(b[0], b[1], false); push(b[0], b[1], true);
        push(a[0], a[1], false); push(b[0], b[1], true); push(a[0], a[1], true);
      }
    }
  }
  const data = { directions: new Float32Array(directions), isTop: new Uint8Array(isTop) };
  slabCache.set(code, data);
  return data;
}

const COLUMN_GEOMETRY = new THREE.CylinderGeometry(COLUMN_RADIUS, COLUMN_RADIUS, 1, 6).translate(0, 0.5, 0);
const UP = new THREE.Vector3(0, 1, 0);

function createPillar(code, material) {
  let mesh;
  // Outlines that cross the date line are skipped by slabData, so those countries get a column too.
  if (isSmall.has(code) || slabData(code).isTop.length === 0) {
    mesh = new THREE.Mesh(COLUMN_GEOMETRY, material);
    const normal = surfacePoint(code, 1);
    mesh.position.copy(normal).multiplyScalar(R * 0.995);
    mesh.quaternion.setFromUnitVectors(UP, normal);
    mesh.userData = { column: true, height: -1 };
  } else {
    const data = slabData(code);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(data.directions.length), 3));
    mesh = new THREE.Mesh(geometry, material);
    mesh.frustumCulled = false;
    mesh.userData = { data, height: -1 };
  }
  setPillarHeight(mesh, 0);
  planet.add(mesh);
  return mesh;
}

function setPillarHeight(mesh, height) {
  if (mesh.userData.height === height) return;
  mesh.userData.height = height;
  if (mesh.userData.column) {
    mesh.scale.set(1, Math.max(0.001, height + 0.012), 1);
    return;
  }
  const { directions, isTop } = mesh.userData.data;
  const positions = mesh.geometry.getAttribute('position');
  for (let index = 0; index < isTop.length; index += 1) {
    const radius = isTop[index] ? R * (1.003 + height) : R * 0.995;
    positions.array[index * 3] = directions[index * 3] * radius;
    positions.array[index * 3 + 1] = directions[index * 3 + 1] * radius;
    positions.array[index * 3 + 2] = directions[index * 3 + 2] * radius;
  }
  positions.needsUpdate = true;
}

function removePillar(mesh) {
  planet.remove(mesh);
  if (!mesh.userData.column) mesh.geometry.dispose();
}

const tierMaterial = tier => new THREE.MeshStandardMaterial({
  color: TIERS[tier - 1].color, emissive: TIERS[tier - 1].color, emissiveIntensity: tier === 5 ? 0.55 : 0.18,
  roughness: 0.45, metalness: 0.05, flatShading: true, side: THREE.DoubleSide,
});
const hoverMaterial = new THREE.MeshStandardMaterial({ color: '#ffffff', emissive: '#ffffff', emissiveIntensity: 0.25, transparent: true, opacity: 0.45, roughness: 0.2, side: THREE.DoubleSide, depthWrite: false });
const wrongMaterial = new THREE.MeshStandardMaterial({ color: WRONG_COLOR, emissive: WRONG_COLOR, emissiveIntensity: 0.5, flatShading: true, side: THREE.DoubleSide });

/* ---------- Effects: shockwave rings, sparks ---------- */
function dotTexture() {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 64;
  const ctx = canvas.getContext('2d');
  const gradient = ctx.createRadialGradient(32, 32, 0, 32, 32, 32);
  gradient.addColorStop(0, 'rgb(255 255 255 / 1)');
  gradient.addColorStop(0.35, 'rgb(255 255 255 / 0.6)');
  gradient.addColorStop(1, 'rgb(255 255 255 / 0)');
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, 64, 64);
  return new THREE.CanvasTexture(canvas);
}
const DOT = dotTexture();
const effects = [];

function shockwave(normal, color, delay = 0) {
  const mesh = new THREE.Mesh(new THREE.RingGeometry(0.86, 1, 72), new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.9, blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.DoubleSide }));
  mesh.position.copy(normal).multiplyScalar(R * 1.008);
  mesh.lookAt(normal.clone().multiplyScalar(3));
  mesh.scale.setScalar(0.001);
  planet.add(mesh);
  effects.push({ start: performance.now() + delay, duration: 1100, update: progress => { mesh.scale.setScalar(0.02 + easeOut(progress) * 0.45); mesh.material.opacity = 0.9 * (1 - progress); }, dispose: () => { planet.remove(mesh); mesh.geometry.dispose(); mesh.material.dispose(); } });
}

function sparkBurst(origin, normal, color, count, power) {
  const positions = new Float32Array(count * 3);
  const velocities = [];
  for (let index = 0; index < count; index += 1) {
    positions.set([origin.x, origin.y, origin.z], index * 3);
    const random = new THREE.Vector3(Math.random() * 2 - 1, Math.random() * 2 - 1, Math.random() * 2 - 1).normalize();
    velocities.push(random.multiplyScalar(0.5).add(normal.clone().multiplyScalar(0.8)).normalize().multiplyScalar(power * (0.4 + Math.random() * 0.8)));
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  const points = new THREE.Points(geometry, new THREE.PointsMaterial({ color, size: 0.05, map: DOT, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }));
  planet.add(points);
  let last = 0;
  effects.push({
    start: performance.now(), duration: 1200,
    update: progress => {
      const step = (progress - last) * 1.2;
      last = progress;
      for (let index = 0; index < count; index += 1) {
        velocities[index].multiplyScalar(1 - 2.2 * step);
        positions[index * 3] += velocities[index].x * step;
        positions[index * 3 + 1] += velocities[index].y * step;
        positions[index * 3 + 2] += velocities[index].z * step;
      }
      geometry.getAttribute('position').needsUpdate = true;
      points.material.opacity = 1 - progress;
    },
    dispose: () => { planet.remove(points); geometry.dispose(); points.material.dispose(); },
  });
}

function runEffects(now) {
  for (let index = effects.length - 1; index >= 0; index -= 1) {
    const effect = effects[index];
    const progress = (now - effect.start) / effect.duration;
    if (progress < 0) continue;
    effect.update(Math.min(1, progress));
    if (progress >= 1) {
      effect.dispose();
      effects.splice(index, 1);
    }
  }
}

/* ---------- Moons: low-poly, with faces that always look at you ---------- */
const moonGroup = new THREE.Group();
worldGroup.add(moonGroup);
// Each tier orbits on its own tilted plane; computed once, reused every frame.
const ORBIT_TILTS = TIERS.map((_, index) => new THREE.Quaternion().setFromEuler(new THREE.Euler(0.32 + (index + 1) * 0.07, 0, -0.3 + (index + 1) * 0.12)));
const orbitLines = new Map();
const moons = [];

function ensureOrbitLine(tier) {
  if (orbitLines.has(tier)) return;
  const radius = R * TIERS[tier - 1].orbit;
  const points = Array.from({ length: 160 }, (_, index) => {
    const angle = (index / 160) * Math.PI * 2;
    return new THREE.Vector3(Math.cos(angle) * radius, 0, Math.sin(angle) * radius);
  });
  const line = new THREE.LineLoop(new THREE.BufferGeometry().setFromPoints(points), new THREE.LineDashedMaterial({ color: TIERS[tier - 1].color, dashSize: 0.04, gapSize: 0.05, transparent: true, opacity: 0.35 }));
  line.computeLineDistances();
  line.quaternion.copy(ORBIT_TILTS[tier - 1]);
  moonGroup.add(line);
  orbitLines.set(tier, line);
}

function orbitPosition(moon, target = new THREE.Vector3()) {
  const radius = R * TIERS[moon.tier - 1].orbit;
  return target.set(Math.cos(moon.angle) * radius, 0, Math.sin(moon.angle) * radius).applyQuaternion(ORBIT_TILTS[moon.tier - 1]);
}

function createMoon(tier, phase) {
  const look = TIERS[tier - 1];
  const size = look.moon;
  const group = new THREE.Group();
  group.add(new THREE.Mesh(new THREE.IcosahedronGeometry(size, 1), new THREE.MeshStandardMaterial({ color: look.color, emissive: look.color, emissiveIntensity: tier === 5 ? 0.6 : 0.12, roughness: 0.6, flatShading: true })));
  const craterMaterial = new THREE.MeshStandardMaterial({ color: new THREE.Color(look.color).multiplyScalar(0.75), roughness: 0.8, flatShading: true });
  [[0.5, -0.45, 0.6], [-0.6, -0.3, 0.6], [0.2, 0.7, -0.6]].forEach(([x, y, z]) => {
    const crater = new THREE.Mesh(new THREE.IcosahedronGeometry(size * 0.22, 0), craterMaterial);
    crater.position.set(x, y, z).normalize().multiplyScalar(size * 0.92);
    crater.scale.z = 0.4;
    crater.lookAt(0, 0, 0);
    group.add(crater);
  });
  const eyes = new THREE.Group();
  const eyeMaterial = new THREE.MeshBasicMaterial({ color: '#13202c' });
  const shineMaterial = new THREE.MeshBasicMaterial({ color: '#ffffff' });
  [-1, 1].forEach(side => {
    const eye = new THREE.Mesh(new THREE.SphereGeometry(size * 0.15, 12, 8), eyeMaterial);
    eye.position.set(side * size * 0.32, size * 0.1, size * 0.92);
    const shine = new THREE.Mesh(new THREE.SphereGeometry(size * 0.05, 8, 6), shineMaterial);
    shine.position.set(size * 0.05, size * 0.06, size * 0.12);
    eye.add(shine);
    eyes.add(eye);
  });
  group.add(eyes);
  moonGroup.add(group);
  ensureOrbitLine(tier);
  const moon = { tier, group, eyes, angle: phase, launch: null, onArrive: null, nextBlink: performance.now() + 1500 + Math.random() * 3000 };
  moons.push(moon);
  return moon;
}

function updateMoons(now, dt) {
  moons.forEach(moon => {
    if (moon.launch) {
      const progress = Math.min(1, (now - moon.launch.start) / LAUNCH_MS);
      const eased = easeOut(progress);
      const end = orbitPosition(moon);
      // Curved flight: out from the pillar, then swinging into the orbit.
      const control = moon.launch.from.clone().add(end).multiplyScalar(0.5).normalize().multiplyScalar(Math.max(moon.launch.from.length(), end.length()) * 1.5);
      const point = new THREE.Vector3()
        .addScaledVector(moon.launch.from, (1 - eased) ** 2)
        .addScaledVector(control, 2 * (1 - eased) * eased)
        .addScaledVector(end, eased ** 2);
      moon.group.position.copy(point);
      moon.group.scale.setScalar(0.2 + 0.8 * eased);
      moon.launch.trailPoints.push(point.clone());
      if (moon.launch.trailPoints.length > 60) moon.launch.trailPoints.shift();
      moon.launch.trail.geometry.setFromPoints(moon.launch.trailPoints);
      if (progress >= 1) arrive(moon);
    } else {
      moon.angle += TIERS[moon.tier - 1].speed * dt;
      orbitPosition(moon, moon.group.position);
    }
    moon.group.lookAt(camera.position);
    if (now > moon.nextBlink) {
      moon.eyes.scale.y = 0.12;
      if (now > moon.nextBlink + 130) {
        moon.eyes.scale.y = 1;
        moon.nextBlink = now + 2500 + Math.random() * 3500;
      }
    }
  });
}

function launchMoon(code, tier, onArrive) {
  const moon = createMoon(tier, Math.random() * Math.PI * 2);
  const from = planet.localToWorld(surfacePoint(code, R * (1.01 + TIERS[tier - 1].height)));
  moon.group.position.copy(from);
  moon.group.scale.setScalar(0.2);
  const trail = new THREE.Line(new THREE.BufferGeometry(), new THREE.LineBasicMaterial({ color: TIERS[tier - 1].color, transparent: true, opacity: 0.9, blending: THREE.AdditiveBlending }));
  worldGroup.add(trail);
  moon.launch = { start: performance.now(), from, trailPoints: [], trail };
  moon.onArrive = onArrive;
  sound.whoosh();
}

// The moon reaches its orbit: the trail fades, sparks fly and the caller shows the banner and points.
function arrive(moon) {
  const { trail } = moon.launch;
  moon.launch = null;
  effects.push({ start: performance.now(), duration: 600, update: progress => { trail.material.opacity = 0.9 * (1 - progress); }, dispose: () => { trail.removeFromParent(); trail.geometry.dispose(); trail.material.dispose(); } });
  const position = moon.group.position.clone();
  sparkBurst(planet.worldToLocal(worldGroup.localToWorld(position.clone())), position.clone().normalize(), TIERS[moon.tier - 1].color, 20 + moon.tier * 8, 0.6);
  sound.pop(moon.tier);
  if (moon.tier >= 4) shake = { start: performance.now(), strength: 0.012 * moon.tier };
  const done = moon.onArrive;
  moon.onArrive = null;
  done?.();
}

/* ---------- HTML tags that follow 3D points ---------- */
const tags = [];
const projected = new THREE.Vector3();
const toPoint = new THREE.Vector3();

function addTag(text, color, getWorldPosition) {
  const element = document.createElement('span');
  element.className = 'tag glass is-new';
  element.style.setProperty('--c', color);
  element.innerHTML = `<i></i>${escapeHtml(text)}`;
  labels.append(element);
  tags.push({ element, getWorldPosition });
}

// A point is hidden when the planet sits between it and the camera.
function occluded(point) {
  toPoint.subVectors(point, camera.position);
  const distance = toPoint.length();
  toPoint.divideScalar(distance);
  const along = -camera.position.dot(toPoint);
  if (along < 0 || along > distance) return false;
  const closest = camera.position.clone().addScaledVector(toPoint, along);
  return closest.length() < R * 0.98;
}

function updateTags() {
  tags.forEach(({ element, getWorldPosition }) => {
    const point = getWorldPosition();
    projected.copy(point).project(camera);
    element.style.transform = `translate(${((projected.x + 1) / 2) * innerWidth}px, ${((1 - projected.y) / 2) * innerHeight}px)`;
    element.classList.toggle('is-hidden', occluded(point) || projected.z > 1);
  });
}

/* ---------- Camera moves, shakes and wobbles ---------- */
let flight = null;
let shake = null;
let wobble = null;
let rising = [];

function flyTo(direction, distance, done = () => {}) {
  const from = camera.position.clone();
  flight = { start: performance.now(), fromDir: from.clone().normalize(), toDir: direction.clone().normalize(), fromDistance: from.length(), toDistance: distance, done };
  controls.enabled = false;
}

function updateFlight(now) {
  if (!flight) return;
  const progress = Math.min(1, (now - flight.start) / FLY_MS);
  const eased = easeInOut(progress);
  const rotation = new THREE.Quaternion().setFromUnitVectors(flight.fromDir, flight.toDir);
  const partial = new THREE.Quaternion().slerp(rotation, eased);
  const lift = 1 + Math.sin(Math.PI * eased) * 0.12;      // pull back a little mid-flight
  camera.position.copy(flight.fromDir).applyQuaternion(partial).multiplyScalar(THREE.MathUtils.lerp(flight.fromDistance, flight.toDistance, eased) * lift);
  camera.lookAt(0, 0, 0);
  if (progress >= 1) {
    const { done } = flight;
    flight = null;
    controls.enabled = true;
    done();
  }
}

function updateRising(now) {
  rising = rising.filter(item => {
    if (item.bounce) {
      const progress = Math.min(1, (now - item.start) / MISS_MS);
      setPillarHeight(item.mesh, item.target * Math.sin(Math.PI * progress));
      if (progress >= 1) { removePillar(item.mesh); return false; }
      return true;
    }
    const progress = Math.min(1, (now - item.start) / RISE_MS);
    setPillarHeight(item.mesh, item.target * easeOutElastic(progress));
    return progress < 1;
  });
}

/* ---------- Placing finds ---------- */
// A found country: its pillar, its tag and its moon. Earlier finds of today are placed without animation.
function placeFind(code, tier, animate, onArrive) {
  const mesh = createPillar(code, tierMaterial(tier));
  const look = TIERS[tier - 1];
  if (animate) rising.push({ mesh, start: performance.now(), target: look.height });
  else setPillarHeight(mesh, look.height);
  addTag(nameOf(code), look.color, () => planet.localToWorld(surfacePoint(code, R * (1.01 + mesh.userData.height))));
  if (animate) {
    const normal = surfacePoint(code, 1);
    for (let index = 0; index < 2 + Math.ceil(tier / 2); index += 1) shockwave(normal, look.color, index * 170);
    sparkBurst(normal.clone().multiplyScalar(R * 1.02), normal, look.color, 30 + tier * 14, 0.5 + tier * 0.12);
    sound.rise(tier);
    setTimeout(() => launchMoon(code, tier, onArrive), LAUNCH_DELAY_MS);
  } else {
    createMoon(tier, moons.length * 2.4 + 0.8);
  }
}

function bonk(code) {
  sound.bonk();
  shockwave(surfacePoint(code, 1), WRONG_COLOR);
  rising.push({ mesh: createPillar(code, wrongMaterial), start: performance.now(), target: 0.05, bounce: true });
  wobble = { start: performance.now() };
}

/* ---------- HUD ---------- */
const tierName = tier => t(`tier.${tier}`);
const currentClue = () => (game.current == null ? null : game.clues[game.current]);
const usedToday = () => new Set(game.clues.flatMap(clue => [clue.country, ...clue.misses].filter(Boolean)));

function renderStatic() {
  const label = (id, key) => document.getElementById(id).setAttribute('aria-label', t(key));
  label('back', 'back'); label('help', 'help'); label('open-settings', 'settings');
  document.querySelector('[data-zoom="in"]').setAttribute('aria-label', t('zoomIn'));
  document.querySelector('[data-zoom="out"]').setAttribute('aria-label', t('zoomOut'));
  document.querySelector('[data-zoom="reset"]').setAttribute('aria-label', t('zoomReset'));
  document.getElementById('score-label').textContent = t('points');
  document.getElementById('input-label').textContent = t('inputLabel');
  document.getElementById('raise').textContent = t('raise');
  document.getElementById('controls-hint').textContent = t('controls');
  document.getElementById('fact-label').textContent = t('factLabel');
  input.placeholder = t('placeholder');
  const playable = countryList.filter(country => regionOk(country.code)).map(country => country.names[lang]).sort((a, b) => a.localeCompare(b, lang));
  document.getElementById('country-list').innerHTML = playable.map(name => `<option value="${escapeHtml(name)}"></option>`).join('');
  updateMuteButton();
}

function renderHud(newIndex = -1) {
  scoreLabel.textContent = game.points;
  findsList.innerHTML = game.clues.map((clue, index) => {
    if (clue.outcome === 'found') {
      return `<li class="glass ${index === newIndex ? 'is-new' : ''}" title="${escapeHtml(clue.clue)}"><i style="--c: ${TIERS[clue.tier - 1].color}"></i>${escapeHtml(nameOf(clue.country))} &middot; ${escapeHtml(tierName(clue.tier))}</li>`;
    }
    if (clue.outcome === 'lost') return `<li class="glass is-lost" title="${escapeHtml(clue.clue)}"><i></i>${escapeHtml(t('lostChip', { n: index + 1 }))}</li>`;
    return `<li class="glass is-empty ${index === game.current ? 'is-current' : ''}"><i></i>${escapeHtml(t('emptyChip', { n: index + 1 }))}</li>`;
  }).join('');
  const clue = currentClue();
  form.hidden = !clue;
  if (!clue) return;
  meta.textContent = t('clueOf', { n: clue.position + 1, total: game.clues.length });
  clueTitle.textContent = clue.clue;
  const lives = Array.from({ length: game.maxMisses }, (_, index) => `<i class="${index < clue.misses.length ? 'is-used' : ''}"></i>`).join('');
  hint.innerHTML = `<span>${escapeHtml(t('fits', { count: clue.validCount }))}</span><span class="lives" aria-hidden="true">${lives}</span>`;
}

function showBanner(tier, points) {
  document.getElementById('banner-title').textContent = t(`banner.${tier}`);
  document.getElementById('banner-sub').textContent = t('plusPoints', { points });
  banner.style.setProperty('--c', TIERS[tier - 1].color);
  restartClass(banner, 'is-on');
}

function showFact(code, tier, fact, onNext) {
  factCard.style.setProperty('--c', TIERS[tier - 1].color);
  document.getElementById('fact-country').textContent = nameOf(code);
  document.getElementById('fact-text').textContent = fact;
  const next = document.getElementById('fact-next');
  next.textContent = game.current == null ? t('seeResults') : t('nextClue');
  next.onclick = () => { factCard.hidden = true; onNext(); };
  factCard.hidden = false;
  next.focus({ preventScroll: true });
}

function renderSummary() {
  const items = game.clues.map(clue => {
    const found = clue.outcome === 'found';
    const lines = [`<b>${escapeHtml(clue.clue)}</b>`];
    lines.push(found
      ? `<span>${escapeHtml(t('yourPick', { name: nameOf(clue.country), tier: tierName(clue.tier), points: clue.points }))}</span>`
      : `<span>${escapeHtml(t('noPick'))}</span>`);
    if (clue.misses.length) lines.push(`<span class="muted">${escapeHtml(t('missed', { list: clue.misses.map(nameOf).join(', ') }))}</span>`);
    if (clue.rarest.length) lines.push(`<span class="muted">${escapeHtml(t('rarestList', { list: clue.rarest.map(nameOf).join(', ') }))}</span>`);
    if (clue.fact) lines.push(`<span>${escapeHtml(clue.fact)}</span>`);
    const moon = found ? `<span class="moon" style="--c: ${TIERS[clue.tier - 1].color}"></span>` : '<span class="moon is-lost"></span>';
    return `<li>${moon}<span class="text">${lines.join('')}</span><span class="points">${clue.points ? `+${clue.points}` : ''}</span></li>`;
  }).join('');
  const moonCount = game.clues.filter(clue => clue.outcome === 'found').length;
  summary.innerHTML = `
    <div class="summary-card glass">
      <h2>${escapeHtml(t('doneTitle'))}</h2>
      <div class="summary-stats"><span>${escapeHtml(t('total'))}: <strong>${game.points}</strong></span><span>${escapeHtml(t('moons'))}: <strong>${moonCount}</strong></span></div>
      <ol class="summary-list">${items}</ol>
      <div class="summary-foot"><span class="countdown" data-countdown></span><span class="row"><button type="button" class="ghost-btn" data-view>${escapeHtml(t('viewPlanet'))}</button><a href="../../">${escapeHtml(t('home'))}</a></span></div>
    </div>`;
  summary.hidden = false;
  meta.textContent = '';
  clueTitle.textContent = t('doneTitle');
  hint.innerHTML = '';
  say(t('done'));
}
// Closing the summary shows the planet with today's moons; the speech bubble brings the summary back.
summary.addEventListener('click', event => {
  if (event.target !== summary && !event.target.closest('[data-view]')) return;
  summary.hidden = true;
  speech.innerHTML = `${escapeHtml(t('done'))} <button type="button" data-results>${escapeHtml(t('seeResults'))}</button>`;
});

// Moves on to the next clue, or to the day's summary.
function advance() {
  busy = false;
  renderHud();
  if (game.status === 'solved') {
    renderSummary();
    return;
  }
  say(t('start'));
  input.focus({ preventScroll: true });
}

/* ---------- Playing ---------- */
function explain(error) {
  return error.code === 'network' ? t('network') : t('jammed');
}

async function tryCountry(code) {
  const clue = currentClue();
  if (!clue || busy || flight) return;
  sound.tap();
  if (!byCode.has(code)) { say(t('notACountry')); return; }
  if (usedToday().has(code)) { say(t('alreadyTried', { name: nameOf(code) })); return; }
  if (!regionOk(code)) { say(t('outsideRegion', { name: nameOf(code), region: t(`region.${game.region}`) })); return; }
  busy = true;
  hideHover();
  let next;
  try {
    next = await api.atlasPick(clue.position, code);
  } catch (error) {
    busy = false;
    say(error.code === 'already_tried' ? t('alreadyTried', { name: nameOf(code) }) : error.code === 'outside_region' ? t('outsideRegion', { name: nameOf(code), region: t(`region.${game.region}`) }) : explain(error));
    return;
  }
  game = next;
  const { result } = next;
  if (result.kind === 'found') {
    say(t('flying', { name: nameOf(code) }));
    flyTo(planet.localToWorld(surfacePoint(code, 1)), CAMERA_DISTANCE, () => {
      say(t('rising', { name: nameOf(code) }));
      placeFind(code, result.tier, true, () => {
        showBanner(result.tier, result.points);
        restartClass(scoreCard, 'is-bump');
        renderHud(clue.position);
        form.hidden = true;
        say(`${t(`line.${result.tier}`)} ${nameOf(code)}: ${t('plusPoints', { points: result.points })}`);
        if (result.fact) showFact(code, result.tier, result.fact, advance);
        else setTimeout(advance, NEXT_CLUE_MS);
      });
    });
    return;
  }
  bonk(code);
  if (result.kind === 'miss') {
    say(t('miss', { name: nameOf(code), count: result.missesLeft }));
    busy = false;
    renderHud();
    return;
  }
  const lostClue = game.clues[clue.position];
  say(t('lost', { rarest: lostClue.rarest.map(nameOf).join(', ') }));
  renderHud();
  form.hidden = true;
  setTimeout(advance, LOST_NEXT_MS);
}

/* ---------- Picking and hovering ---------- */
const raycaster = new THREE.Raycaster();
const pointer = new THREE.Vector2();
let hovered = null;
let hoverMesh = null;

function countryAt(clientX, clientY) {
  pointer.set((clientX / innerWidth) * 2 - 1, -(clientY / innerHeight) * 2 + 1);
  raycaster.setFromCamera(pointer, camera);
  const hit = raycaster.intersectObject(ocean, false)[0];
  if (!hit) return null;
  const [lon, lat] = vecToLonLat(planet.worldToLocal(hit.point.clone()));
  for (const [code, [[west, south], [east, north]]] of boundsOf) {
    const inLon = west <= east ? lon >= west && lon <= east : lon >= west || lon <= east;
    if (inLon && lat >= south && lat <= north && d3.geoContains(featureOf.get(code), [lon, lat])) return code;
  }
  const near = smallCountries.find(country => d3.geoDistance([lon, lat], [country.latlng[1], country.latlng[0]]) < SMALL_PICK_DEG * DEG);
  return near?.code ?? null;
}

function hideHover() {
  if (hoverMesh) removePillar(hoverMesh);
  hoverMesh = null;
  hovered = null;
  stage.classList.remove('is-hovering');
}

function updateHover(clientX, clientY) {
  const code = countryAt(clientX, clientY);
  if (code === hovered) return;
  hideHover();
  if (!code || !currentClue() || busy || usedToday().has(code) || !regionOk(code)) return;
  hovered = code;
  stage.classList.add('is-hovering');
  hoverMesh = createPillar(code, hoverMaterial);
  setPillarHeight(hoverMesh, 0.012);
}

let press = null;
let hoverQueued = null;
renderer.domElement.addEventListener('pointerdown', event => {
  sound.ensure();
  press = { x: event.clientX, y: event.clientY };
});
renderer.domElement.addEventListener('pointermove', event => {
  if (press || flight) return;
  hoverQueued = { x: event.clientX, y: event.clientY };
});
renderer.domElement.addEventListener('pointerup', event => {
  if (!press) return;
  const moved = Math.hypot(event.clientX - press.x, event.clientY - press.y);
  press = null;
  if (moved > CLICK_SLOP_PX || flight) return;
  const code = countryAt(event.clientX, event.clientY);
  if (code) tryCountry(code);
});
renderer.domElement.addEventListener('pointerleave', hideHover);

function findCountry(typed) {
  const wanted = normalizeName(typed, lang);
  if (!wanted) return null;
  const names = country => [country.names[lang], country.names.en, ...country.aliases].map(name => normalizeName(name, lang));
  return countryList.find(country => names(country).includes(wanted))
    ?? countryList.find(country => normalizeName(country.names[lang], lang).startsWith(wanted))
    ?? null;
}

form.addEventListener('submit', event => {
  event.preventDefault();
  sound.ensure();
  const text = input.value.trim();
  if (!text) return;
  const match = findCountry(text);
  if (!match) {
    say(t('unknownName', { text }));
    return;
  }
  input.value = '';
  tryCountry(match.code);
});

document.getElementById('zoom').addEventListener('click', event => {
  const action = event.target.closest('[data-zoom]')?.dataset.zoom;
  if (action === 'in') camera.position.setLength(clamp(camera.position.length() / ZOOM_STEP, MIN_DISTANCE, MAX_DISTANCE));
  else if (action === 'out') camera.position.setLength(clamp(camera.position.length() * ZOOM_STEP, MIN_DISTANCE, MAX_DISTANCE));
  else if (action === 'reset') flyTo(cameraHome(), CAMERA_DISTANCE);
});

function updateMuteButton() {
  muteButton.setAttribute('aria-pressed', String(sound.muted));
  muteButton.setAttribute('aria-label', sound.muted ? t('unmute') : t('mute'));
}
muteButton.addEventListener('click', () => {
  sound.muted = !sound.muted;
  storage.set(MUTE_KEY, sound.muted ? '1' : '0');
  updateMuteButton();
});

/* ---------- Dialogs ---------- */
function openSettings() {
  const settingsForm = document.createElement('form');
  settingsForm.className = 'settings';
  const current = settings.atlas;
  settingsForm.innerHTML = `
    <fieldset>
      <legend>${escapeHtml(t('region'))}</legend>
      ${REGIONS.map(region => `<label class="option"><input type="radio" name="region" value="${region}" ${current.region === region ? 'checked' : ''}>${escapeHtml(t(`region.${region}`))}</label>`).join('')}
    </fieldset>
    <p class="help">${escapeHtml(t('regionHelp'))}</p>
    <fieldset>
      <legend>${escapeHtml(t('facts'))}</legend>
      <label class="option"><input type="checkbox" name="facts" ${current.facts ? 'checked' : ''}>${escapeHtml(t('factsOn'))}</label>
    </fieldset>
    <p class="settings-error" role="alert" hidden></p>`;
  const errorLine = settingsForm.querySelector('.settings-error');
  settingsForm.addEventListener('submit', event => event.preventDefault());
  settingsForm.addEventListener('change', async () => {
    const previous = settings.atlas;
    const wanted = { region: settingsForm.elements.region.value, facts: settingsForm.elements.facts.checked };
    try {
      settings = await api.saveAtlasSettings(wanted);
      errorLine.hidden = true;
      if (settings.atlas.region !== game.region) say(t('regionNext', { region: t(`region.${settings.atlas.region}`) }));
    } catch {
      settingsForm.elements.region.value = previous.region;
      settingsForm.elements.facts.checked = previous.facts;
      errorLine.textContent = t('saveFailed');
      errorLine.hidden = false;
    }
  });
  showDialog({ title: t('settings'), content: settingsForm, closeLabel: t('close') });
}
document.getElementById('help').addEventListener('click', () => showDialog({ title: t('help'), content: `<p>${escapeHtml(t('rules'))}</p>`, closeLabel: t('close') }));
document.getElementById('open-settings').addEventListener('click', openSettings);

window.addEventListener('resize', () => {
  camera.aspect = innerWidth / innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(innerWidth, innerHeight);
  composer.setSize(innerWidth, innerHeight);
  bloom.setSize(innerWidth, innerHeight);
});

/* ---------- Start ---------- */
renderStatic();
if (game.status === 'waiting') {
  meta.textContent = '';
  clueTitle.textContent = 'Atlas Hunt';
  say(t('waiting'));
} else {
  game.clues.filter(clue => clue.outcome === 'found').forEach(clue => placeFind(clue.country, clue.tier, false));
  renderHud();
  if (game.status === 'solved') renderSummary();
  else say(game.nextRegion !== game.region ? `${t('start')} ${t('regionNext', { region: t(`region.${game.nextRegion}`) })}` : t('start'));
}
startCountdown(game.secondsToReset, left => {
  const element = summary.querySelector('[data-countdown]');
  if (element) element.textContent = t('nextIn', { time: formatDuration(left) });
}, () => location.reload());

/* ---------- Main loop ---------- */
const clock = new THREE.Clock();
const sunWorld = new THREE.Vector3();
renderer.setAnimationLoop(() => {
  const dt = Math.min(0.05, clock.getDelta());
  const now = performance.now();
  oceanUniforms.uTime.value += dt;
  cloudLayer.rotation.y += dt * 0.025;
  updateFlight(now);
  if (!flight) controls.update();
  sun.getWorldPosition(sunWorld);
  oceanUniforms.uSun.value.copy(sunWorld).normalize();
  if (hoverQueued) {
    updateHover(hoverQueued.x, hoverQueued.y);
    hoverQueued = null;
  }
  updateRising(now);
  updateMoons(now, dt);
  runEffects(now);
  worldGroup.position.set(0, 0, 0);
  if (shake) {
    const progress = (now - shake.start) / SHAKE_MS;
    if (progress >= 1) shake = null;
    else worldGroup.position.set((Math.random() - 0.5) * shake.strength * (1 - progress), (Math.random() - 0.5) * shake.strength * (1 - progress), 0);
  }
  worldGroup.rotation.z = 0;
  if (wobble) {
    const progress = (now - wobble.start) / SHAKE_MS;
    if (progress >= 1) wobble = null;
    else worldGroup.rotation.z = Math.sin(progress * Math.PI * 6) * 0.06 * (1 - progress);
  }
  composer.render();
  updateTags();
});
