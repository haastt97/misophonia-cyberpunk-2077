"use strict";

const pageSize = 40;
const filters = document.querySelector("#filters");
const query = document.querySelector("#query");
const sounds = document.querySelector("#sounds");
const notice = document.querySelector("#notice");
const resultCount = document.querySelector("#result-count");
const bottomPager = document.querySelector("#bottom-pagination");
const previewPanel = document.querySelector("#now-playing");
const player = document.querySelector("#audio");
const playbackError = document.querySelector("#playback-error");
const pageButtons = Array.from(document.querySelectorAll("[data-page]"));
const formatter = new Intl.NumberFormat();

const state = {
  categories: [],
  categoriesById: new Map(),
  kinds: new Map(),
  items: [],
  total: 0,
  offset: 0,
  request: null,
  ready: false,
  standIns: new Map(),
  expanded: new Set(),
  saving: new Set(),
  activeSound: null,
  audioAttempt: 0,
};

function element(name, className, text) {
  const node = document.createElement(name);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function option(value, text) {
  const node = element("option", "", text);
  node.value = value;
  return node;
}

function categoryName(id) {
  const category = state.categoriesById.get(id);
  if (!category) return id;
  return category.section ? `${category.section} · ${category.name}` : category.name;
}

function addCategoryOptions(select) {
  const groups = new Map();
  for (const category of state.categories) {
    let group = groups.get(category.section);
    if (!group) {
      group = element("optgroup");
      group.label = category.section || "General";
      groups.set(category.section, group);
      select.append(group);
    }
    group.append(option(category.id, category.name));
  }
}

async function api(path, init) {
  const response = await fetch(path, init);
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    throw new Error(body.error || `Request failed (${response.status}).`);
  }
  return response.json();
}

function showNotice(message = "", error = false) {
  notice.textContent = message;
  notice.hidden = !message;
  if (error) notice.dataset.error = "true";
  else delete notice.dataset.error;
}

function updatePager() {
  const loading = Boolean(state.request);
  for (const button of pageButtons) {
    button.disabled = loading || (button.dataset.page === "previous"
      ? state.offset === 0
      : state.offset + pageSize >= state.total);
  }
  bottomPager.hidden = state.total <= pageSize;
  sounds.setAttribute("aria-busy", String(loading));
  if (loading) {
    resultCount.textContent = "Loading sounds…";
  } else if (!state.total) {
    resultCount.textContent = "No matching sounds";
  } else {
    const start = formatter.format(state.offset + 1);
    const end = formatter.format(Math.min(state.offset + pageSize, state.total));
    resultCount.textContent = `${start}–${end} of ${formatter.format(state.total)} results`;
  }
}

async function loadPage(offset = 0, focusResults = false) {
  if (!state.ready) return;
  state.request?.abort();
  const request = new AbortController();
  state.request = request;
  const params = new URLSearchParams(new FormData(filters));
  params.set("limit", String(pageSize));
  params.set("offset", String(offset));
  updatePager();
  try {
    const result = await api(`/api/sounds?${params}`, { signal: request.signal });
    if (state.request !== request) return;
    if (offset >= result.total && offset >= pageSize) {
      const previous = Math.max(0, Math.floor((result.total - 1) / pageSize) * pageSize);
      return loadPage(previous, focusResults);
    }
    state.items = result.items;
    state.total = result.total;
    state.offset = offset;
    renderResults();
    if (focusResults) {
      sounds.focus({ preventScroll: true });
      document.querySelector(".result-bar").scrollIntoView({ block: "start" });
    }
  } catch (error) {
    if (!request.signal.aborted) showNotice(error.message, true);
  } finally {
    if (state.request === request) {
      state.request = null;
      updatePager();
    }
  }
}

function modeName(mode) {
  if (mode === "slider" || mode === "volume") return "volume slider";
  if (mode === "switch" || mode === "mute" || mode === "toggle") return "mute";
  return mode;
}

function coverageName(coverage) {
  const edition = coverage.edition === "runtime" ? "Configurable"
    : coverage.edition === "standalone" ? "Standalone" : coverage.edition;
  return [edition, modeName(coverage.mode)].filter(Boolean).join(" ");
}

function makeEventDetails(item) {
  const detail = element("details");
  const count = item.events.length;
  detail.append(element("summary", "", `${formatter.format(count)} linked ${count === 1 ? "event" : "events"}`));
  function renderEvents() {
    if (detail.querySelector("ul")) return;
    const list = element("ul", "event-list");
    const eventCoverage = new Map();
    for (const entry of item.coverage) {
      for (const event of entry.events || []) {
        if (!eventCoverage.has(event)) eventCoverage.set(event, []);
        eventCoverage.get(event).push(`${categoryName(entry.category)} · ${coverageName(entry)}`);
      }
    }
    for (const event of item.events) {
      const row = element("li");
      row.append(element("code", "", event));
      const labels = eventCoverage.get(event);
      if (labels?.length) row.append(element("small", "event-coverage", labels.join(" / ")));
      list.append(row);
    }
    detail.append(list);
  }
  detail.open = state.expanded.has(item.id);
  if (detail.open) renderEvents();
  detail.addEventListener("toggle", () => {
    if (!detail.isConnected) return;
    if (detail.open) {
      state.expanded.add(item.id);
      renderEvents();
    } else {
      state.expanded.delete(item.id);
    }
  });
  return detail;
}

function actionButton(action, className, label, text) {
  const button = element("button", className, text);
  button.type = "button";
  button.dataset.action = action;
  button.setAttribute("aria-label", label);
  button.title = label;
  return button;
}

function makeCategorySelect(labelText, action, value, disabled) {
  const label = element("label", "", labelText);
  const select = element("select");
  select.dataset.action = action;
  select.append(option("", "Choose category"));
  addCategoryOptions(select);
  select.value = value || "";
  select.disabled = disabled;
  label.append(select);
  return label;
}

function startStandInRequest(category) {
  const record = { items: null, error: false };
  state.standIns.set(category, record);
  const params = new URLSearchParams({ category, exclude: "" });
  api(`/api/stand-ins?${params}`).then(({ items }) => {
    record.items = items;
  }).catch(() => {
    record.error = true;
  }).finally(() => {
    if (state.standIns.get(category) === record) refreshChoices(category);
  });
  return record;
}

function updateChoices(select, item) {
  const { candidate, category, replacement } = item.tags;
  const record = candidate && category
    ? state.standIns.get(category) || startStandInRequest(category)
    : null;
  select.replaceChildren(option("", "Mute (default)"));
  if (record?.items) {
    for (const standIn of record.items) {
      if (standIn.id !== item.id) select.append(option(standIn.id, standIn.name));
    }
  }
  if (replacement && !Array.from(select.options).some((choice) => choice.value === replacement)) {
    const placeholder = !record?.items && !record?.error ? "Saved stand-in (loading…)" : "Saved stand-in (unavailable)";
    select.append(option(replacement, placeholder));
  }
  if (record && !record.items) {
    const status = option("__status__", record.error ? "Could not load stand-ins" : "Loading stand-ins…");
    status.disabled = true;
    select.append(status);
  }
  select.value = replacement || "";
  select.disabled = !candidate || state.saving.has(item.id);
}

function refreshChoices(category) {
  for (const row of sounds.children) {
    const item = state.items.find((entry) => entry.id === row.dataset.sound);
    if (item && (!category || item.tags.category === category)) {
      updateChoices(row.querySelector('[data-action="replacement"]'), item);
    }
  }
}

function makeSound(item) {
  const row = element("article", "sound");
  row.dataset.sound = item.id;
  const head = element("div", "sound-head");
  const play = actionButton("play", "play", `Play ${item.name}`);
  play.disabled = item.playable === false;
  if (play.disabled) {
    play.setAttribute("aria-label", `No installed audio for ${item.name}`);
    play.title = "This event has no linked installed audio";
  }

  const info = element("div");
  const name = element("h2", "sound-name", item.name);
  if (item.path) name.title = item.path;
  const meta = element("p", "sound-meta");
  meta.append(element("span", "sound-kind", state.kinds.get(item.kind) || item.kind));
  if (item.detail) meta.append(element("span", "", item.detail));
  info.append(name, meta);

  if (item.coverage.length) {
    const coverage = element("ul", "coverage");
    coverage.setAttribute("aria-label", "Current mod coverage");
    for (const entry of item.coverage) {
      const badge = element("li", "", `${categoryName(entry.category)} `);
      if (entry.mode === "slider" || entry.mode === "volume") badge.dataset.mode = "slider";
      badge.append(element("small", "", `· ${coverageName(entry)}`));
      coverage.append(badge);
    }
    info.append(coverage);
  } else {
    info.append(element("p", "uncovered", "No linked mod coverage"));
  }
  if (item.events.length) info.append(makeEventDetails(item));

  const buttons = element("div", "tag-buttons");
  const saving = state.saving.has(item.id);
  const candidate = actionButton("candidate", "tag-button", `Candidate tag for ${item.name}`, "X");
  candidate.setAttribute("aria-pressed", String(item.tags.candidate));
  candidate.disabled = saving;
  const standIn = actionButton("stand-in", "tag-button", `Stand-in tag for ${item.name}`, "O");
  standIn.setAttribute("aria-pressed", String(item.tags.stand_in));
  standIn.disabled = saving || item.playable === false;
  if (item.playable === false) standIn.title = "A stand-in needs playable audio";
  buttons.append(candidate, standIn);

  const fields = element("div", "tag-fields");
  fields.dataset.standIn = String(item.tags.stand_in);
  fields.append(makeCategorySelect("Candidate category", "candidate-category", item.tags.category, saving || !item.tags.candidate));
  const replacementLabel = element("label", "", "Replacement");
  const replacement = element("select");
  replacement.dataset.action = "replacement";
  updateChoices(replacement, item);
  replacementLabel.append(replacement);
  fields.append(replacementLabel);
  if (item.tags.stand_in) {
    fields.append(makeCategorySelect("Stand-in category", "stand-in-category", item.tags.stand_in_category, saving));
  }
  head.append(play, info, buttons);
  row.append(head, fields);
  return row;
}

function restoreFocus(row, action) {
  if (!action) return;
  const control = Array.from(row.querySelectorAll("[data-action]")).find((node) => node.dataset.action === action);
  if (control && !control.disabled) control.focus({ preventScroll: true });
}

function renderResults() {
  const { sound, action } = {
    sound: document.activeElement.closest("[data-sound]")?.dataset.sound,
    action: document.activeElement.dataset.action,
  };
  sounds.replaceChildren();
  if (!state.items.length) {
    sounds.append(element("p", "empty", "No sounds match these filters."));
    return;
  }
  for (const item of state.items) {
    const row = makeSound(item);
    sounds.append(row);
    if (item.id === sound) restoreFocus(row, action);
  }
  updatePlayback();
}

function updateSound(item, focusAction) {
  const old = Array.from(sounds.children).find((row) => row.dataset.sound === item.id);
  if (!old) return;
  const row = makeSound(item);
  old.replaceWith(row);
  restoreFocus(row, focusAction);
  updatePlayback();
}

function updatePlayback() {
  for (const row of sounds.children) {
    const button = row.querySelector('[data-action="play"]');
    if (!button || button.disabled) continue;
    const item = state.items.find((entry) => entry.id === row.dataset.sound);
    const playing = state.activeSound === item.id && !player.paused && !player.ended;
    row.dataset.playing = String(playing);
    button.setAttribute("aria-pressed", String(playing));
    button.setAttribute("aria-label", `${playing ? "Pause" : "Play"} ${item.name}`);
    button.title = playing ? "Pause" : "Play";
  }
}

function displayPlaybackError() {
  if (!state.activeSound) return;
  playbackError.textContent = "This preview could not be played. Check the local tool’s output.";
  playbackError.hidden = false;
  updatePlayback();
}

async function playSound(item) {
  if (state.activeSound === item.id && !player.paused) {
    player.pause();
    return;
  }
  const attempt = ++state.audioAttempt;
  playbackError.hidden = true;
  if (state.activeSound !== item.id) {
    player.pause();
    state.activeSound = item.id;
    player.src = `/api/audio/${encodeURIComponent(item.id)}`;
    document.querySelector("#playing-name").textContent = item.name;
    previewPanel.hidden = false;
    document.body.classList.add("has-preview");
  } else if (player.error) {
    player.load();
  }
  try {
    await player.play();
  } catch (error) {
    if (attempt === state.audioAttempt && error.name !== "AbortError") displayPlaybackError();
  }
}

async function saveTags(item, tags, action) {
  if (state.saving.has(item.id)) return;
  const previous = item.tags;
  state.saving.add(item.id);
  updateSound(item);
  showNotice();
  try {
    const result = await api(`/api/sounds/${encodeURIComponent(item.id)}/tags`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(tags),
    });
    item.tags = result.tags;
    const activeItem = state.items.find((entry) => entry.id === item.id);
    if (activeItem) activeItem.tags = result.tags;
    if (previous.stand_in !== result.tags.stand_in || previous.stand_in_category !== result.tags.stand_in_category) {
      state.standIns.clear();
      refreshChoices();
    }
    if (result.reset_count) {
      const noun = result.reset_count === 1 ? "replacement" : "replacements";
      showNotice(`${result.reset_count} ${noun} reset to mute.`);
    }
    const activeFilters = new FormData(filters);
    if (activeFilters.get("filter") !== "all" || activeFilters.get("category") || result.reset_count) {
      await loadPage(state.offset);
    }
  } catch (error) {
    showNotice(`Could not save tags for ${item.name}. ${error.message}`, true);
  } finally {
    state.saving.delete(item.id);
    const activeItem = state.items.find((entry) => entry.id === item.id);
    if (activeItem) updateSound(activeItem, document.activeElement === document.body ? action : undefined);
  }
}

function itemForControl(control) {
  const id = control.closest("[data-sound]")?.dataset.sound;
  return state.items.find((entry) => entry.id === id);
}

sounds.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-action]");
  if (!button || button.disabled) return;
  const item = itemForControl(button);
  if (!item) return;
  const { action } = button.dataset;
  if (action === "play") {
    playSound(item);
  } else if (action === "candidate") {
    const candidate = !item.tags.candidate;
    saveTags(item, { ...item.tags, candidate, category: candidate ? item.tags.stand_in_category : null, replacement: null }, action);
  } else if (action === "stand-in") {
    const standIn = !item.tags.stand_in;
    saveTags(item, { ...item.tags, stand_in: standIn, stand_in_category: standIn ? item.tags.category : null }, action);
  }
});

sounds.addEventListener("change", (event) => {
  const select = event.target.closest("select[data-action]");
  if (!select || select.disabled) return;
  const item = itemForControl(select);
  if (!item) return;
  const { action } = select.dataset;
  const value = select.value || null;
  const tags = { ...item.tags };
  if (action === "candidate-category") {
    tags.category = value;
    tags.replacement = null;
  } else if (action === "replacement") {
    tags.replacement = value;
  } else if (action === "stand-in-category") {
    tags.stand_in_category = value;
  }
  saveTags(item, tags, action);
});

for (const eventName of ["play", "pause", "ended"]) player.addEventListener(eventName, updatePlayback);
player.addEventListener("error", displayPlaybackError);
player.volume = 0.35;
document.querySelector("#close-player").addEventListener("click", () => {
  const previous = state.activeSound;
  state.activeSound = null;
  state.audioAttempt++;
  player.pause();
  player.removeAttribute("src");
  player.load();
  previewPanel.hidden = true;
  document.body.classList.remove("has-preview");
  updatePlayback();
  const row = Array.from(sounds.children).find((entry) => entry.dataset.sound === previous);
  if (row) restoreFocus(row, "play");
});

let searchTimer;
query.addEventListener("input", () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => loadPage(0), 220);
});
filters.addEventListener("submit", (event) => {
  event.preventDefault();
  clearTimeout(searchTimer);
  loadPage(0);
});
filters.addEventListener("change", (event) => {
  if (event.target === query) return;
  clearTimeout(searchTimer);
  loadPage(0);
});
for (const button of pageButtons) {
  button.addEventListener("click", () => {
    const direction = button.dataset.page === "previous" ? -1 : 1;
    loadPage(state.offset + direction * pageSize, true);
  });
}

async function initialize() {
  try {
    const catalog = await api("/api/catalog");
    state.categories = catalog.categories;
    state.categoriesById = new Map(catalog.categories.map((category) => [category.id, category]));
    state.kinds = new Map(catalog.kinds.map((kind) => [kind.id, kind.name]));
    addCategoryOptions(document.querySelector("#category-filter"));
    const kindFilter = document.querySelector("#kind-filter");
    for (const [id, name] of state.kinds) kindFilter.append(option(id, name));
    const summary = ["Cyberpunk 2077", `${formatter.format(catalog.stats.sounds)} sounds`];
    if (catalog.version) summary.push(`Mod ${catalog.version}`);
    document.querySelector("#library-summary").textContent = summary.join(" · ");
    if (catalog.warnings?.length) showNotice(catalog.warnings.join(" "));
    state.ready = true;
    await loadPage();
  } catch (error) {
    sounds.setAttribute("aria-busy", "false");
    resultCount.textContent = "Library unavailable";
    showNotice(error.message, true);
  }
}

initialize();
