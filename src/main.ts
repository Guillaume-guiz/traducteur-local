import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { accelToDisplay, buildShortcut } from "./raccourcis";

type Lang = { code: string; en: string; label: string };

const LANGS: Lang[] = [
  { code: "fr", en: "French", label: "Français" },
  { code: "en", en: "English", label: "Anglais" },
  { code: "es", en: "Spanish", label: "Espagnol" },
  { code: "de", en: "German", label: "Allemand" },
  { code: "it", en: "Italian", label: "Italien" },
  { code: "pt", en: "Portuguese", label: "Portugais" },
  { code: "nl", en: "Dutch", label: "Néerlandais" },
  { code: "zh", en: "Chinese", label: "Chinois" },
  { code: "ja", en: "Japanese", label: "Japonais" },
  { code: "ru", en: "Russian", label: "Russe" },
  { code: "ar", en: "Arabic", label: "Arabe" },
];

const state = {
  source: "auto",
  target: "en",
  tone: "neutral",
  model: "",
};

// --- petits helpers DOM ---
const $ = <T extends HTMLElement>(sel: string): T =>
  document.querySelector(sel) as T;

const sourceEl = $<HTMLTextAreaElement>("#source");
const outputEl = $<HTMLElement>("#target");
const errorEl = $<HTMLElement>("#error");
const spinnerEl = $<HTMLElement>("#spinner");
const latencyEl = $<HTMLElement>("#latency");
const statusEl = $<HTMLElement>("#status");
const modelSel = $<HTMLSelectElement>("#model-select");
const sourceSel = $<HTMLSelectElement>("#source-lang");
const targetSel = $<HTMLSelectElement>("#target-lang");
const swapBtn = $<HTMLButtonElement>("#swap");
const countEl = $<HTMLElement>("#source-count");
const clearBtn = $<HTMLButtonElement>("#clear");
const copyBtn = $<HTMLButtonElement>("#copy-btn");
const insertBtn = $<HTMLButtonElement>("#insert-btn");
const rephraseBtn = $<HTMLButtonElement>("#rephrase-btn");
const rephraseMenu = $<HTMLElement>("#rephrase-menu");
const altBtn = $<HTMLButtonElement>("#alt-btn");
const altPanel = $<HTMLElement>("#alt-panel");
const altList = $<HTMLElement>("#alt-list");
const altClose = $<HTMLButtonElement>("#alt-close");
const a11yBanner = $<HTMLElement>("#a11y-banner");
const a11yOpen = $<HTMLButtonElement>("#a11y-open");
const settingsBtn = $<HTMLButtonElement>("#settings-btn");
const settingsOverlay = $<HTMLElement>("#settings-overlay");
const settingsClose = $<HTMLButtonElement>("#settings-close");
const settingsCancel = $<HTMLButtonElement>("#settings-cancel");
const settingsSave = $<HTMLButtonElement>("#settings-save");
const shortcutField = $<HTMLButtonElement>("#shortcut-field");
const shortcutReset = $<HTMLButtonElement>("#shortcut-reset");
const shortcutError = $<HTMLElement>("#shortcut-error");
const hintShortcut = $<HTMLElement>("#hint-shortcut");
const tabShortcut = $<HTMLButtonElement>("#tab-shortcut");
const tabModels = $<HTMLButtonElement>("#tab-models");
const panelShortcut = $<HTMLElement>("#panel-shortcut");
const panelModels = $<HTMLElement>("#panel-models");
const sysinfoEl = $<HTMLElement>("#sysinfo");
const modelListEl = $<HTMLElement>("#model-list");
const modelError = $<HTMLElement>("#model-error");

const enName = (code: string): string =>
  LANGS.find((l) => l.code === code)?.en ?? "English";

function setOutput(text: string) {
  outputEl.textContent = text;
  const has = text.trim().length > 0;
  copyBtn.disabled = !has;
  insertBtn.disabled = !has;
  rephraseBtn.disabled = !has;
  altBtn.disabled = !has;
}

function showSpinner(on: boolean) {
  spinnerEl.classList.toggle("hidden", !on);
}

function showError(msg: string) {
  errorEl.textContent = msg;
  errorEl.classList.remove("hidden");
}
function clearError() {
  errorEl.classList.add("hidden");
  errorEl.textContent = "";
}

// --- traduction (avec anti-collision de requêtes) ---
let reqToken = 0;
let debounceTimer: number | undefined;

async function runTranslate() {
  const text = sourceEl.value;
  const token = ++reqToken;
  clearError();
  if (!text.trim()) {
    setOutput("");
    latencyEl.textContent = "";
    return;
  }
  showSpinner(true);
  const started = performance.now();
  try {
    const res = await invoke<string>("translate", {
      text,
      sourceLang: state.source,
      targetLang: enName(state.target),
      tone: state.tone,
      model: state.model,
    });
    if (token !== reqToken) return; // une frappe plus récente a pris le relais
    setOutput(res);
    latencyEl.textContent = `${((performance.now() - started) / 1000).toFixed(1)} s`;
  } catch (e) {
    if (token !== reqToken) return;
    setOutput("");
    showError(String(e));
  } finally {
    if (token === reqToken) showSpinner(false);
  }
}

function scheduleTranslate(delay = 650) {
  window.clearTimeout(debounceTimer);
  debounceTimer = window.setTimeout(runTranslate, delay);
}

// --- reformulation (DeepL Write) ---
async function runRephrase(mode: string) {
  const text = outputEl.textContent ?? "";
  if (!text.trim()) return;
  rephraseMenu.classList.add("hidden");
  showSpinner(true);
  clearError();
  try {
    const res = await invoke<string>("rephrase", {
      text,
      mode,
      model: state.model,
    });
    setOutput(res);
  } catch (e) {
    showError(String(e));
  } finally {
    showSpinner(false);
  }
}

// --- variantes ---
function currentSelectionOrOutput(): string {
  const sel = window.getSelection();
  if (
    sel &&
    sel.toString().trim() &&
    sel.anchorNode &&
    outputEl.contains(sel.anchorNode)
  ) {
    return sel.toString();
  }
  return outputEl.textContent ?? "";
}

async function runAlternatives() {
  const text = currentSelectionOrOutput();
  if (!text.trim()) return;
  altPanel.classList.remove("hidden");
  altList.innerHTML = `<div class="alt-loading">Génération des variantes…</div>`;
  try {
    const res = await invoke<string[]>("alternatives", {
      text,
      targetLang: enName(state.target),
      model: state.model,
    });
    altList.innerHTML = "";
    if (!res.length) {
      altList.innerHTML = `<div class="alt-loading">Aucune variante.</div>`;
      return;
    }
    for (const variant of res) {
      const b = document.createElement("button");
      b.className = "alt-item";
      b.textContent = variant;
      b.addEventListener("click", () => {
        setOutput(variant);
        copyText(variant);
        altPanel.classList.add("hidden");
      });
      altList.appendChild(b);
    }
  } catch (e) {
    altList.innerHTML = `<div class="alt-loading">${String(e)}</div>`;
  }
}

// --- copier ---
async function copyText(t: string) {
  try {
    await navigator.clipboard.writeText(t);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = t;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    ta.remove();
  }
  copyBtn.textContent = "Copié ✓";
  window.setTimeout(() => (copyBtn.textContent = "Copier"), 1200);
}

// --- insérer dans le champ d'origine (réinjection façon DeepL) ---
async function runInsert() {
  const t = outputEl.textContent ?? "";
  if (!t.trim()) return;
  try {
    await invoke<boolean>("insert_translation", { text: t });
    a11yBanner.classList.add("hidden");
  } catch (e) {
    if (String(e).includes("ACCESSIBILITY_REQUIRED")) {
      // La traduction est déjà dans le presse-papier (copiée côté Rust avant la
      // vérif) : on le signale, et on propose d'activer l'insertion auto.
      flashInsert("Copié ✓ — colle avec ⌘V");
      a11yBanner.classList.remove("hidden");
      invoke("request_accessibility"); // pop-up macOS native + inscrit l'app dans la liste
    } else {
      showError(String(e));
    }
  }
}

function flashInsert(label: string) {
  insertBtn.textContent = label;
  window.setTimeout(() => (insertBtn.textContent = "Insérer ⏎"), 1800);
}

// --- réglages : raccourci global personnalisable ---
const DEFAULT_ACCEL = "CmdOrCtrl+Shift+T";
let currentAccel = DEFAULT_ACCEL; // raccourci actif (format accélérateur Tauri)
let pendingAccel: string | null = null; // capturé, pas encore enregistré
let capturing = false;

function setCapturing(on: boolean) {
  capturing = on;
  shortcutField.classList.toggle("capturing", on);
  if (on) shortcutField.textContent = "Appuie sur ta combinaison…";
}

function openSettings() {
  // revient toujours sur l'onglet Raccourci à l'ouverture
  tabShortcut.classList.add("is-active");
  tabModels.classList.remove("is-active");
  panelShortcut.classList.remove("hidden");
  panelModels.classList.add("hidden");
  shortcutError.classList.add("hidden");
  shortcutError.textContent = "";
  pendingAccel = null;
  setCapturing(false);
  shortcutField.textContent = accelToDisplay(currentAccel);
  settingsSave.disabled = true;
  settingsOverlay.classList.remove("hidden");
}

function closeSettings() {
  if (capturing) {
    setCapturing(false);
    invoke("resume_shortcut");
  }
  settingsOverlay.classList.add("hidden");
}

async function beginCapture() {
  if (capturing) return;
  await invoke("pause_shortcut"); // l'ancien raccourci ne doit pas se déclencher
  setCapturing(true);
}

function onCaptureKeydown(e: KeyboardEvent) {
  if (!capturing) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") {
    setCapturing(false);
    invoke("resume_shortcut");
    shortcutField.textContent = accelToDisplay(pendingAccel ?? currentAccel);
    return;
  }
  const built = buildShortcut(e);
  if (!built) return; // attend une combinaison valide (modificateur + touche)
  pendingAccel = built.accel;
  shortcutField.textContent = built.display;
  setCapturing(false);
  invoke("resume_shortcut"); // réactive l'ancien jusqu'à « Enregistrer »
  shortcutError.classList.add("hidden");
  settingsSave.disabled = built.accel === currentAccel;
}

function resetShortcut() {
  pendingAccel = DEFAULT_ACCEL;
  shortcutField.textContent = accelToDisplay(DEFAULT_ACCEL);
  shortcutError.classList.add("hidden");
  settingsSave.disabled = DEFAULT_ACCEL === currentAccel;
}

async function saveShortcut() {
  if (!pendingAccel) return;
  try {
    const applied = await invoke<string>("set_shortcut", { shortcut: pendingAccel });
    currentAccel = applied;
    hintShortcut.textContent = accelToDisplay(applied);
    shortcutField.textContent = accelToDisplay(applied);
    pendingAccel = null;
    settingsSave.disabled = true;
    settingsOverlay.classList.add("hidden");
  } catch (e) {
    shortcutError.textContent = String(e);
    shortcutError.classList.remove("hidden");
  }
}

async function initShortcut() {
  try {
    const cur = await invoke<string>("get_shortcut");
    currentAccel = cur || DEFAULT_ACCEL;
  } catch {
    currentAccel = DEFAULT_ACCEL;
  }
  hintShortcut.textContent = accelToDisplay(currentAccel);
}

// --- gestion des modèles (catalogue + reco selon la config matérielle) ---
type CatalogModel = {
  tag: string;
  label: string;
  sizeGb: number; // poids du téléchargement
  ramGb: number; // RAM approximative en charge (quantization Q4 par défaut)
  note: string;
};

// Catalogue curaté, orienté traduction (modèles multilingues de qualité).
const MODEL_CATALOG: CatalogModel[] = [
  { tag: "aya-expanse:8b", label: "Aya Expanse 8B", sizeGb: 5.1, ramGb: 6, note: "Multilingue Cohere (23 langues)" },
  { tag: "gemma2:9b", label: "Gemma 2 9B", sizeGb: 5.4, ramGb: 7, note: "Google, excellent FR/EN" },
  { tag: "qwen2.5:7b", label: "Qwen 2.5 7B", sizeGb: 4.7, ramGb: 6, note: "Rapide, solide multilingue" },
  { tag: "mistral-nemo:12b", label: "Mistral Nemo 12B", sizeGb: 7.1, ramGb: 9, note: "Contexte long, multilingue" },
  { tag: "qwen2.5:14b", label: "Qwen 2.5 14B", sizeGb: 9.0, ramGb: 11, note: "Très bon rapport qualité/taille" },
  { tag: "gemma2:27b", label: "Gemma 2 27B", sizeGb: 16, ramGb: 18, note: "Qualité supérieure" },
  { tag: "aya-expanse:32b", label: "Aya Expanse 32B", sizeGb: 18, ramGb: 20, note: "Le meilleur multilingue (lourd)" },
];

type SysInfo = { ram_gb: number; free_disk_gb: number; chip: string; arch: string };
type PullProgress = { model: string; status: string; completed: number; total: number; percent: number };
type ModelStatus = { kind: "installed" | "recommended" | "tight" | "blocked"; reason?: string };

const DISK_SAFETY_GB = 12; // marge de disque libre à préserver
const DISK_MIN_FREE_GB = 5; // en-dessous, on bloque carrément
const BADGE: Record<ModelStatus["kind"], string> = {
  installed: "🟢 Installé",
  recommended: "✅ Recommandé",
  tight: "⚠️ Disque juste",
  blocked: "❌ Trop lourd",
};

let sysInfo: SysInfo | null = null;
let installedModels: string[] = [];
let pulling = false;

function showModelError(msg: string) {
  modelError.textContent = msg;
  modelError.classList.remove("hidden");
}
function clearModelError() {
  modelError.classList.add("hidden");
  modelError.textContent = "";
}

// Reconstruit le dropdown du haut à partir des modèles installés.
function syncModelDropdown() {
  modelSel.innerHTML = "";
  for (const m of installedModels) {
    const o = document.createElement("option");
    o.value = m;
    o.textContent = m;
    modelSel.appendChild(o);
  }
  if (state.model && installedModels.includes(state.model)) {
    modelSel.value = state.model;
  } else if (installedModels.length) {
    state.model = installedModels[0];
    modelSel.value = state.model;
  } else {
    state.model = "";
  }
}

function modelStatus(m: CatalogModel): ModelStatus {
  if (installedModels.includes(m.tag)) return { kind: "installed" };
  if (!sysInfo) return { kind: "recommended" };
  if (m.ramGb > sysInfo.ram_gb - 2) {
    return { kind: "blocked", reason: `RAM insuffisante (~${m.ramGb} Go requis)` };
  }
  const leftover = sysInfo.free_disk_gb - m.sizeGb;
  if (leftover < DISK_MIN_FREE_GB) {
    return { kind: "blocked", reason: `Disque insuffisant (${m.sizeGb} Go à télécharger)` };
  }
  if (leftover < DISK_SAFETY_GB) {
    return { kind: "tight", reason: `Te laisserait ~${Math.round(leftover)} Go de disque` };
  }
  return { kind: "recommended" };
}

async function refreshModelsData() {
  try {
    installedModels = await invoke<string[]>("list_models");
  } catch {
    installedModels = [];
  }
  if (!sysInfo) {
    try {
      sysInfo = await invoke<SysInfo>("system_info");
    } catch {
      sysInfo = null;
    }
  }
}

function renderSysInfo() {
  if (!sysInfo) {
    sysinfoEl.textContent = "Configuration indisponible.";
    return;
  }
  sysinfoEl.innerHTML = `<b>${sysInfo.chip}</b> · ${sysInfo.ram_gb} Go RAM · ${sysInfo.free_disk_gb} Go disque libre`;
}

function renderModels() {
  renderSysInfo();
  modelListEl.innerHTML = "";

  // Modèles installés hors catalogue (ex : mistral:latest) → gérables aussi.
  const catalogTags = new Set(MODEL_CATALOG.map((m) => m.tag));
  const extras: CatalogModel[] = installedModels
    .filter((t) => !catalogTags.has(t))
    .map((t) => ({ tag: t, label: t, sizeGb: 0, ramGb: 0, note: "Installé (hors catalogue)" }));

  for (const m of [...MODEL_CATALOG, ...extras]) {
    const st = modelStatus(m);
    const isActive = state.model === m.tag;
    const card = document.createElement("div");
    card.className =
      "model-card" + (st.kind === "blocked" ? " is-blocked" : "") + (isActive ? " is-active" : "");

    const info = document.createElement("div");
    info.className = "model-info";
    const sizeStr = m.sizeGb ? ` · ${m.sizeGb} Go` : "";
    info.innerHTML =
      `<div class="model-name">${m.label}${isActive ? ' <span class="model-active">● actif</span>' : ""}</div>` +
      `<div class="model-meta">${BADGE[st.kind]}${sizeStr}</div>` +
      `<div class="model-note">${st.reason ?? m.note}</div>`;
    card.appendChild(info);

    const actions = document.createElement("div");
    actions.className = "model-actions";
    if (st.kind === "installed") {
      if (!isActive) {
        const use = document.createElement("button");
        use.className = "ghost";
        use.textContent = "Utiliser";
        use.addEventListener("click", () => selectModel(m.tag));
        actions.appendChild(use);
      }
      const del = document.createElement("button");
      del.className = "ghost danger";
      del.textContent = "Supprimer";
      del.addEventListener("click", () => removeModel(m.tag, del));
      actions.appendChild(del);
    } else if (st.kind === "blocked") {
      const b = document.createElement("button");
      b.className = "ghost";
      b.textContent = "Installer";
      b.disabled = true;
      actions.appendChild(b);
    } else {
      const inst = document.createElement("button");
      inst.className = "primary";
      inst.textContent = "Installer";
      inst.addEventListener("click", () => installModel(m.tag, inst));
      actions.appendChild(inst);
    }
    card.appendChild(actions);
    modelListEl.appendChild(card);
  }
}

function selectModel(tag: string) {
  if (!installedModels.includes(tag)) installedModels.push(tag);
  state.model = tag;
  syncModelDropdown();
  invoke("set_saved_model", { model: tag });
  renderModels();
  if (sourceEl.value.trim()) runTranslate();
}

async function installModel(tag: string, btn: HTMLButtonElement) {
  if (pulling) return;
  pulling = true;
  clearModelError();
  btn.disabled = true;
  const card = btn.closest<HTMLElement>(".model-card");
  const bar = document.createElement("div");
  bar.className = "model-progress";
  bar.innerHTML =
    `<div class="model-progress-bar"><span></span></div>` +
    `<div class="model-progress-label">Démarrage…</div>`;
  card?.appendChild(bar);
  const fill = bar.querySelector<HTMLElement>("span")!;
  const label = bar.querySelector<HTMLElement>(".model-progress-label")!;

  const unlisten = await listen<PullProgress>("pull-progress", (e) => {
    if (e.payload.model !== tag) return;
    const p = e.payload;
    fill.style.width = `${p.percent}%`;
    const gb = p.total
      ? ` (${(p.completed / 1e9).toFixed(1)}/${(p.total / 1e9).toFixed(1)} Go)`
      : "";
    label.textContent = `${p.status}${gb}`;
  });

  try {
    await invoke("pull_model", { name: tag });
    await refreshModelsData();
    selectModel(tag); // déclenche un re-render (la barre disparaît)
  } catch (err) {
    bar.classList.add("error");
    label.textContent = String(err);
    btn.disabled = false;
  } finally {
    unlisten();
    pulling = false;
  }
}

// Suppression à double-clic (confirmation inline, car window.confirm n'est pas fiable sous Tauri).
function removeModel(tag: string, btn: HTMLButtonElement) {
  if (btn.dataset.confirm !== "1") {
    btn.dataset.confirm = "1";
    btn.textContent = "Confirmer ?";
    window.setTimeout(() => {
      if (btn.dataset.confirm === "1") {
        btn.dataset.confirm = "";
        btn.textContent = "Supprimer";
      }
    }, 2500);
    return;
  }
  doRemoveModel(tag);
}

async function doRemoveModel(tag: string) {
  clearModelError();
  try {
    await invoke("delete_model", { name: tag });
    const wasActive = state.model === tag;
    await refreshModelsData();
    if (wasActive) {
      state.model = installedModels[0] ?? "";
      if (state.model) invoke("set_saved_model", { model: state.model });
    }
    syncModelDropdown();
    renderModels();
  } catch (err) {
    showModelError(String(err));
  }
}

function showTab(which: "shortcut" | "models") {
  const isModels = which === "models";
  tabShortcut.classList.toggle("is-active", !isModels);
  tabModels.classList.toggle("is-active", isModels);
  panelShortcut.classList.toggle("hidden", isModels);
  panelModels.classList.toggle("hidden", !isModels);
  if (isModels) openModelsTab();
}

async function openModelsTab() {
  clearModelError();
  sysinfoEl.textContent = "Détection de la configuration…";
  modelListEl.innerHTML = `<div class="alt-loading">Chargement des modèles…</div>`;
  await refreshModelsData();
  renderModels();
}

// --- statut Ollama + liste des modèles ---
async function initModels() {
  try {
    installedModels = await invoke<string[]>("list_models");
    if (!installedModels.length) {
      statusEl.textContent = "Aucun modèle";
      statusEl.className = "status-pill ko";
      modelSel.innerHTML = "";
      return;
    }
    let saved = "";
    try {
      saved = await invoke<string>("get_saved_model");
    } catch {
      saved = "";
    }
    state.model =
      (saved && installedModels.includes(saved) ? saved : undefined) ??
      installedModels.find((m) => m.startsWith("aya-expanse")) ??
      installedModels.find((m) => m.startsWith("gemma")) ??
      installedModels[0];
    syncModelDropdown();
    statusEl.textContent = "Connecté";
    statusEl.className = "status-pill ok";
  } catch (e) {
    statusEl.textContent = "Ollama hors ligne";
    statusEl.className = "status-pill ko";
    showError(
      "Impossible de joindre Ollama sur http://localhost:11434. Lance Ollama puis recharge.",
    );
  }
}

// --- dernière dictée, versée par la pastille ---
type LastDictation = { dit: string; texte: string; cible: string };

function showDictation(d: LastDictation) {
  if (!d.dit.trim()) return;
  reqToken++; // une traduction en cours ne doit pas écraser la dictée
  window.clearTimeout(debounceTimer);
  clearError();
  sourceEl.value = d.dit;
  const n = d.dit.length;
  countEl.textContent = `${n} caractère${n > 1 ? "s" : ""}`;
  // « Simplified Chinese » côté dictée, « Chinese » ici.
  const lang = LANGS.find((l) => l.en === d.cible || d.cible.endsWith(l.en));
  if (d.cible === "none" || !lang) {
    // Dictée sans traduction : on la traduit vers la langue choisie ici.
    runTranslate();
    return;
  }
  state.target = lang.code;
  targetSel.value = lang.code;
  showSpinner(false);
  setOutput(d.texte);
  latencyEl.textContent = "";
}

// --- sélecteurs de langue ---
function fillLangSelects() {
  const auto = document.createElement("option");
  auto.value = "auto";
  auto.textContent = "Détection auto";
  sourceSel.appendChild(auto);
  for (const l of LANGS) {
    const o1 = document.createElement("option");
    o1.value = l.code;
    o1.textContent = l.label;
    sourceSel.appendChild(o1);
    const o2 = document.createElement("option");
    o2.value = l.code;
    o2.textContent = l.label;
    targetSel.appendChild(o2);
  }
  sourceSel.value = state.source;
  targetSel.value = state.target;
}

function updateSwapState() {
  swapBtn.disabled = state.source === "auto";
}

// --- câblage des événements ---
function wire() {
  sourceEl.addEventListener("input", () => {
    const n = sourceEl.value.length;
    countEl.textContent = `${n} caractère${n > 1 ? "s" : ""}`;
    scheduleTranslate();
  });

  // ⌘⏎ ou Ctrl+⏎ pour traduire tout de suite
  sourceEl.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      window.clearTimeout(debounceTimer);
      runTranslate();
    }
  });

  modelSel.addEventListener("change", () => {
    state.model = modelSel.value;
    invoke("set_saved_model", { model: modelSel.value });
    if (sourceEl.value.trim()) runTranslate();
  });

  sourceSel.addEventListener("change", () => {
    state.source = sourceSel.value;
    updateSwapState();
    if (sourceEl.value.trim()) runTranslate();
  });

  targetSel.addEventListener("change", () => {
    state.target = targetSel.value;
    if (sourceEl.value.trim()) runTranslate();
  });

  swapBtn.addEventListener("click", () => {
    if (state.source === "auto") return;
    const oldSource = state.source;
    state.source = state.target;
    state.target = oldSource;
    sourceSel.value = state.source;
    targetSel.value = state.target;
    // renvoie la traduction dans l'entrée
    const out = outputEl.textContent ?? "";
    if (out.trim()) {
      sourceEl.value = out;
      sourceEl.dispatchEvent(new Event("input"));
    } else {
      runTranslate();
    }
  });

  document.querySelectorAll<HTMLButtonElement>(".tone").forEach((btn) => {
    btn.addEventListener("click", () => {
      document
        .querySelectorAll(".tone")
        .forEach((b) => b.classList.remove("is-active"));
      btn.classList.add("is-active");
      state.tone = btn.dataset.tone ?? "neutral";
      if (sourceEl.value.trim()) runTranslate();
    });
  });

  clearBtn.addEventListener("click", () => {
    sourceEl.value = "";
    sourceEl.dispatchEvent(new Event("input"));
    setOutput("");
    latencyEl.textContent = "";
    sourceEl.focus();
  });

  copyBtn.addEventListener("click", () => copyText(outputEl.textContent ?? ""));
  insertBtn.addEventListener("click", runInsert);
  a11yOpen.addEventListener("click", () => invoke("open_accessibility_settings"));

  // réglages : ouverture/fermeture + capture du raccourci
  settingsBtn.addEventListener("click", openSettings);
  settingsClose.addEventListener("click", closeSettings);
  settingsCancel.addEventListener("click", closeSettings);
  settingsSave.addEventListener("click", saveShortcut);
  shortcutField.addEventListener("click", beginCapture);
  shortcutReset.addEventListener("click", resetShortcut);
  tabShortcut.addEventListener("click", () => showTab("shortcut"));
  tabModels.addEventListener("click", () => showTab("models"));
  // capture en phase « capture » pour précéder les autres handlers clavier
  document.addEventListener("keydown", onCaptureKeydown, true);
  settingsOverlay.addEventListener("click", (e) => {
    if (e.target === settingsOverlay) closeSettings();
  });
  // ouverture depuis l'item « Modifier le raccourci… » de l'icône menu bar
  listen("open-settings", () => openSettings());

  rephraseBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    rephraseMenu.classList.toggle("hidden");
  });
  rephraseMenu.querySelectorAll<HTMLButtonElement>("button").forEach((b) => {
    b.addEventListener("click", () => runRephrase(b.dataset.mode ?? "improve"));
  });

  altBtn.addEventListener("click", runAlternatives);
  altClose.addEventListener("click", () => altPanel.classList.add("hidden"));

  // fermer le menu reformuler au clic extérieur
  document.addEventListener("click", (e) => {
    if (
      !rephraseMenu.contains(e.target as Node) &&
      e.target !== rephraseBtn
    ) {
      rephraseMenu.classList.add("hidden");
    }
  });

  // ⌘⏎ global sur la sortie pour insérer rapidement
  document.addEventListener("keydown", (e) => {
    if (capturing || !settingsOverlay.classList.contains("hidden")) return;
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && document.activeElement !== sourceEl) {
      if (!insertBtn.disabled) {
        e.preventDefault();
        runInsert();
      }
    }
  });

  // pastille agrandie : la dernière dictée arrive dans les deux panneaux,
  // sans nouvelle traduction (celle qui a été collée est déjà là)
  listen<LastDictation>("derniere-dictee", (event) => showDictation(event.payload));

  // raccourci de traduction changé depuis le panneau de la dictée
  listen<string>("raccourci-traduction", (event) => {
    currentAccel = event.payload;
    hintShortcut.textContent = accelToDisplay(currentAccel);
  });

  // raccourci global : texte poussé depuis le presse-papier
  listen<string>("clipboard-translate", (event) => {
    const text = event.payload ?? "";
    if (!text.trim()) return;
    sourceEl.value = text;
    sourceEl.dispatchEvent(new Event("input"));
    window.clearTimeout(debounceTimer);
    runTranslate();
  });
}

window.addEventListener("DOMContentLoaded", () => {
  fillLangSelects();
  updateSwapState();
  wire();
  initModels();
  initShortcut();
  sourceEl.focus();
});
