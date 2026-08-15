/*
 * PROTOTYPE / THROWAWAY.
 * Five typography treatments of one fixed Gallery split composition. The
 * question is which open pair can carry the prototype's editorial voice.
 */

const variants = {
  A: {
    name: "Selected preferred",
    barLabel: "Preferred · Palatino / Segoe UI",
    displayName: "Palatino Linotype",
    utilityName: "Segoe UI",
    displayFamily: '"Palatino Linotype", Georgia, serif',
    utilityFamily: '"Segoe UI", sans-serif',
    displayWeight: "400",
    displayTracking: "-0.045em",
    displayLeading: "0.98",
    titleSize: "clamp(3.3rem, 4.6vw, 10.5rem)",
    longTitleSize: "clamp(2.5rem, 3.65vw, 8rem)",
    license: "System-font comparison reference",
  },
  B: {
    name: "Newsreader / Inter",
    barLabel: "Newsreader / Inter",
    displayName: "Newsreader",
    utilityName: "Inter",
    displayFamily: '"Newsreader", Georgia, serif',
    utilityFamily: '"Inter", sans-serif',
    displayWeight: "430",
    displayTracking: "-0.04em",
    displayLeading: "0.98",
    titleSize: "clamp(3.3rem, 4.6vw, 10.5rem)",
    longTitleSize: "clamp(2.5rem, 3.65vw, 8rem)",
    license: "Open source via Google Fonts",
  },
  C: {
    name: "Source Serif 4 / Source Sans 3",
    barLabel: "Source Serif 4 / Source Sans 3",
    displayName: "Source Serif 4",
    utilityName: "Source Sans 3",
    displayFamily: '"Source Serif 4", Georgia, serif',
    utilityFamily: '"Source Sans 3", sans-serif',
    displayWeight: "410",
    displayTracking: "-0.042em",
    displayLeading: "0.97",
    titleSize: "clamp(3.3rem, 4.6vw, 10.5rem)",
    longTitleSize: "clamp(2.5rem, 3.65vw, 8rem)",
    license: "Open source via Google Fonts",
  },
  D: {
    name: "Selected fallback",
    barLabel: "Fallback · Libre Baskerville / IBM Plex Sans",
    displayName: "Libre Baskerville",
    utilityName: "IBM Plex Sans",
    displayFamily: '"Libre Baskerville", Georgia, serif',
    utilityFamily: '"IBM Plex Sans", sans-serif',
    displayWeight: "400",
    displayTracking: "-0.043em",
    displayLeading: "1.02",
    titleSize: "clamp(3.05rem, 4.2vw, 9.7rem)",
    longTitleSize: "clamp(2.3rem, 3.3vw, 7.4rem)",
    license: "Open source via Google Fonts",
  },
  E: {
    name: "Cormorant Garamond / Work Sans",
    barLabel: "Cormorant Garamond / Work Sans",
    displayName: "Cormorant Garamond",
    utilityName: "Work Sans",
    displayFamily: '"Cormorant Garamond", Georgia, serif',
    utilityFamily: '"Work Sans", sans-serif',
    displayWeight: "500",
    displayTracking: "-0.025em",
    displayLeading: "0.91",
    titleSize: "clamp(3.65rem, 5vw, 11.3rem)",
    longTitleSize: "clamp(2.75rem, 4vw, 8.8rem)",
    license: "Open source via Google Fonts",
  },
};

const release = {
  title: "Last Light on Phobos",
  artist: "Evelyn Lark & The Orbital Choir",
  album: "Signals from the Quiet Sea",
  artwork: {
    path: "album-art.svg",
    alt: "Synthetic cover for Signals from the Quiet Sea",
  },
};

const identity = {
  trackedOutput: "AudioDevice",
  trackedZone: "Living Room",
};

const position = {
  elapsedSeconds: 171,
  durationSeconds: 266,
  elapsedLabel: "2:51",
  remainingLabel: "−1:35",
  fraction: 0.643,
};

const states = {
  playing: {
    label: "Playing",
    playback: "playing",
    track: release,
    position,
    ...identity,
  },
  paused: {
    label: "Paused",
    playback: "paused",
    track: release,
    position,
    ...identity,
  },
  loadingTrack: {
    label: "Loading with metadata",
    playback: "loading",
    track: release,
    position,
    ...identity,
  },
  long: {
    label: "Long metadata",
    playback: "playing",
    track: {
      ...release,
      title: "A Cartographer’s Guide to the Constellations We Invented While Waiting for Dawn",
      artist: "The Evelyn Lark Trans-Orbital Broadcast Ensemble",
      album: "Field Recordings from the Far Side of an Unfinished Memory",
    },
    position,
    ...identity,
  },
  idle: {
    label: "Idle",
    playback: "idle",
    heading: "Nothing is playing",
    track: null,
    position: null,
    ...identity,
  },
  loading: {
    label: "Loading",
    playback: "loading",
    heading: "Loading",
    track: null,
    position: null,
    ...identity,
  },
  missingArtwork: {
    label: "Missing artwork",
    playback: "playing",
    track: { ...release, artwork: null },
    position,
    ...identity,
  },
  missingDetails: {
    label: "Missing details",
    playback: "playing",
    heading: "Now Playing details unavailable",
    track: null,
    position: null,
    ...identity,
  },
  pairingRequired: {
    label: "Pairing required",
    statusLabel: "Pairing required",
    playback: "pairing-required",
    heading: "Enable RoonScape",
    explanation: "Open Settings → Extensions in a Roon client, then enable RoonScape.",
    track: null,
    position: null,
    trackedOutput: null,
    trackedZone: null,
  },
  disconnected: {
    label: "Disconnected",
    playback: "disconnected",
    heading: "Waiting for Roon",
    explanation: "Check Roon Server and the network. This display updates when Roon returns.",
    track: null,
    position: null,
    trackedOutput: null,
    trackedZone: null,
  },
  outputUnavailable: {
    label: "Output unavailable",
    statusLabel: "Output unavailable",
    playback: "output-unavailable",
    heading: "Tracked Output unavailable",
    explanation: "Configure a Tracked Output on this RoonScape host, or check that the selected output is available in Roon.",
    track: null,
    position: null,
    trackedOutput: null,
    trackedZone: null,
  },
};

const variantKeys = Object.keys(variants);
const stateKeys = Object.keys(states);
const params = new URLSearchParams(window.location.search);
let variantKey = variants[params.get("variant")] ? params.get("variant") : "A";
let stateKey = states[params.get("state")] ? params.get("state") : "playing";

const app = document.querySelector("#app");
const inspector = document.querySelector("#fixture-inspector");
const inspectorJson = document.querySelector("#fixture-json");
const inspectorButton = document.querySelector("#inspect-fixture");

function statusMarkup(state) {
  const statusLabel = state.statusLabel ?? state.playback[0].toUpperCase() + state.playback.slice(1);
  return `<span class="status" data-kind="${state.playback}"><span class="status-dot" aria-hidden="true"></span>${statusLabel}</span>`;
}

function identityMarkup(state) {
  if (!state.trackedOutput || !state.trackedZone) return "";
  return `<div class="identity" aria-label="Tracked output and zone">
    <span><b>Output:</b>${state.trackedOutput}</span>
    <span><b>Zone:</b>${state.trackedZone}</span>
  </div>`;
}

function progressMarkup(state) {
  if (!state.position) return "";
  const percent = (state.position.fraction * 100).toFixed(1);
  return `<div class="progress" style="--progress:${percent}%">
    <div class="progress-track" role="progressbar" aria-label="Track progress" aria-valuemin="0" aria-valuemax="${state.position.durationSeconds}" aria-valuenow="${state.position.elapsedSeconds}">
      <span class="progress-fill"></span>
    </div>
    <div class="times"><span>${state.position.elapsedLabel}</span><span>${state.position.remainingLabel}</span></div>
  </div>`;
}

function artworkMarkup(track) {
  if (!track.artwork) return `<div class="artwork-missing" aria-label="Artwork unavailable"></div>`;
  return `<img class="artwork" src="${track.artwork.path}" alt="${track.artwork.alt}" />`;
}

function gallerySplit(state) {
  const longClass = stateKey === "long" ? " is-long" : "";
  return `<main class="screen gallery-split${longClass}" aria-label="${state.label} Gallery split presentation">
    <div class="artwork-wrap">${artworkMarkup(state.track)}</div>
    <section class="metadata-column">
      <div class="metadata-stack">
        ${statusMarkup(state)}
        <h1 class="title">${state.track.title}</h1>
        ${state.track.artist ? `<p class="artist">${state.track.artist}</p>` : ""}
        ${state.track.album ? `<p class="album">${state.track.album}</p>` : ""}
        ${progressMarkup(state)}
      </div>
      ${identityMarkup(state)}
    </section>
  </main>`;
}

function fullField(state) {
  return `<main class="screen full-field" aria-label="${state.label} presentation">
    <section class="full-copy">
      ${statusMarkup(state)}
      <h1>${state.heading}</h1>
      ${state.explanation ? `<p class="explanation">${state.explanation}</p>` : ""}
    </section>
    ${identityMarkup(state)}
  </main>`;
}

function applyVariant(variant) {
  const style = document.documentElement.style;
  style.setProperty("--display-family", variant.displayFamily);
  style.setProperty("--utility-family", variant.utilityFamily);
  style.setProperty("--display-weight", variant.displayWeight);
  style.setProperty("--display-tracking", variant.displayTracking);
  style.setProperty("--display-leading", variant.displayLeading);
  style.setProperty("--title-size", variant.titleSize);
  style.setProperty("--long-title-size", variant.longTitleSize);
}

function writeUrl() {
  const nextParams = new URLSearchParams(window.location.search);
  nextParams.set("variant", variantKey);
  nextParams.set("state", stateKey);
  history.replaceState(null, "", `${window.location.pathname}?${nextParams.toString()}`);
}

function resolvedFont(selector) {
  const element = document.querySelector(selector);
  return element ? getComputedStyle(element).fontFamily : null;
}

function updateInspector() {
  const variant = variants[variantKey];
  const state = states[stateKey];
  inspectorJson.textContent = JSON.stringify(
    {
      typographyDecision: "A preferred; use D as a complete pair when either preferred face is unavailable",
      variantKey,
      typography: variant,
      declaredDisplayFont: resolvedFont(".title, .full-field h1"),
      declaredUtilityFont: resolvedFont(".status"),
      fixtureKey: stateKey,
      fixture: state,
    },
    null,
    2,
  );
}

function render() {
  const variant = variants[variantKey];
  const state = states[stateKey];
  applyVariant(variant);
  app.innerHTML = state.track ? gallerySplit(state) : fullField(state);
  document.querySelector("#variant-key").textContent = variantKey;
  document.querySelector("#variant-name").textContent = variant.barLabel;
  document.querySelector("#cycle-state").textContent = `State: ${state.label}`;
  document.title = `${variantKey} — ${variant.name} · RoonScape font study`;
  writeUrl();
  updateInspector();
}

function cycle(list, current, direction) {
  const index = list.indexOf(current);
  return list[(index + direction + list.length) % list.length];
}

function changeVariant(direction) {
  variantKey = cycle(variantKeys, variantKey, direction);
  render();
}

function changeState(direction = 1) {
  stateKey = cycle(stateKeys, stateKey, direction);
  render();
}

function toggleInspector() {
  inspector.hidden = !inspector.hidden;
  inspectorButton.setAttribute("aria-expanded", String(!inspector.hidden));
  inspectorButton.textContent = inspector.hidden ? "Inspect" : "Hide inspector";
}

document.querySelector("#previous-variant").addEventListener("click", () => changeVariant(-1));
document.querySelector("#next-variant").addEventListener("click", () => changeVariant(1));
document.querySelector("#cycle-state").addEventListener("click", () => changeState(1));
inspectorButton.addEventListener("click", toggleInspector);

window.addEventListener("keydown", (event) => {
  if (event.target.matches("input, textarea, [contenteditable]")) return;
  if (event.key === "ArrowLeft") changeVariant(-1);
  if (event.key === "ArrowRight") changeVariant(1);
  if (event.key === "ArrowUp") changeState(-1);
  if (event.key === "ArrowDown") changeState(1);
  if (event.key.toLowerCase() === "i") toggleInspector();
});

window.addEventListener("popstate", () => {
  const nextParams = new URLSearchParams(window.location.search);
  variantKey = variants[nextParams.get("variant")] ? nextParams.get("variant") : "A";
  stateKey = states[nextParams.get("state")] ? nextParams.get("state") : "playing";
  render();
});

render();
document.fonts.ready.then(updateInspector);
