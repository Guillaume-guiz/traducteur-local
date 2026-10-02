// Panneau de dictée, ouvert au clic sur l'icône de la barre de menus ou au
// clic droit sur la pastille : trois boutons pour choisir la langue de
// sortie, un bouton Réglages (dont les raccourcis clavier).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { accelToDisplay, buildShortcut } from "./raccourcis";

type Target = { value: string; label: string; code: string; icon: string };
type WarmChoice = { secs: number; label: string };
type SonChoice = { value: string; label: string };
type Action = "dictee" | "suivante" | "precedente" | "traduction";
type Jeu = Record<Action, string>;
type Correction = { de: string; vers: string };
type Config = {
  current: string;
  quick: string[];
  targets: Target[];
  raccourcis: { actuels: Jeu; defauts: Jeu };
  lexique: Correction[];
  warm_secs: number;
  warm_choices: WarmChoice[];
  son: string;
  son_choices: SonChoice[];
};

const root = document.getElementById("app") as HTMLElement;
let cfg: Config | null = null;
let view: "quick" | "setup" | "lexique" = "quick";
// Saisie en cours dans « Mots à corriger » (gardée d'un affichage à l'autre).
let saisieDe = "";
let saisieVers = "";
let erreurLexique = "";
// Raccourci en cours de saisie, et message d'erreur de la dernière saisie.
let capture: Action | null = null;
let controleSeul = false;
let erreurRaccourci = "";

const ACTIONS: [Action, string][] = [
  ["dictee", "Dicter (maintenir)"],
  ["suivante", "Langue suivante"],
  ["precedente", "Langue précédente"],
  ["traduction", "Traduire la sélection"],
];

const estControle = (v: string) => /^(control|ctrl)$/i.test(v.trim());
const afficherRaccourci = (v: string) => (estControle(v) ? "⌃ seul" : accelToDisplay(v));
const texteDictee = (c: Config) => {
  const v = c.raccourcis.actuels.dictee;
  return estControle(v) ? "Maintiens Contrôle pour dicter" : `Maintiens ${accelToDisplay(v)} pour dicter`;
};

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string);

const findTarget = (value: string) => cfg?.targets.find((t) => t.value === value);

async function load() {
  cfg = await invoke<Config>("get_dictation_config");
  render();
}

function quickView(c: Config): string {
  const tiles = c.quick
    .map((value, i) => {
      const t = findTarget(value);
      if (!t) return "";
      const active = value === c.current;
      return `<button class="tile${active ? " active" : ""}" data-value="${esc(t.value)}" aria-pressed="${active}" title="Touche ${i + 1}">
        <span class="icon" aria-hidden="true">${t.icon}</span>
        <span class="name">${esc(t.label)}</span>
        <span class="num" aria-hidden="true">${i + 1}</span>
      </button>`;
    })
    .join("");
  const current = findTarget(c.current);
  const elsewhere =
    current && !c.quick.includes(c.current)
      ? `<p class="note">Langue actuelle : ${esc(current.label)}</p>`
      : "";
  return `
    <header class="head">
      <h1>Dictée</h1>
      <p class="hint">${esc(texteDictee(c))}</p>
    </header>
    <div class="tiles" role="group" aria-label="Langue de sortie">${tiles}</div>
    ${elsewhere}
    <footer class="foot">
      <button class="btn" id="open-translator">Traducteur</button>
      <button class="btn" id="to-setup">Réglages</button>
    </footer>`;
}

function options(c: Config, selected: string): string {
  return c.targets
    .map(
      (t) =>
        `<option value="${esc(t.value)}"${t.value === selected ? " selected" : ""}>${t.icon}  ${esc(t.label)}</option>`,
    )
    .join("");
}

function raccourcisSection(c: Config): string {
  const lignes = ACTIONS.map(([action, libelle]) => {
    const enSaisie = capture === action;
    const texte = enSaisie ? "Tape..." : afficherRaccourci(c.raccourcis.actuels[action]);
    return `
      <div class="field ligne-raccourci">
        <span>${esc(libelle)}</span>
        <button class="touche${enSaisie ? " capture" : ""}" data-raccourci="${action}">${esc(texte)}</button>
      </div>`;
  }).join("");
  const erreur = erreurRaccourci ? `<p class="help erreur">${esc(erreurRaccourci)}</p>` : "";
  return `
    <section class="group">
      <h2>Raccourcis clavier</h2>
      ${lignes}
      ${erreur}
      <p class="help">Clique sur un raccourci, puis tape la nouvelle combinaison. Échap annule, ⌫ remet celui d'origine. Pour dicter : Contrôle seul, ou une combinaison, à maintenir pendant que tu parles.</p>
    </section>`;
}

function setupView(c: Config): string {
  const selects = c.quick
    .map(
      (value, i) => `
      <label class="field">
        <span>Bouton ${i + 1}</span>
        <select data-i="${i}">${options(c, value)}</select>
      </label>`,
    )
    .join("");
  const warm = c.warm_choices
    .map(
      (w) =>
        `<option value="${w.secs}"${w.secs === c.warm_secs ? " selected" : ""}>${esc(w.label)}</option>`,
    )
    .join("");
  const son = c.son_choices
    .map(
      (s) =>
        `<option value="${esc(s.value)}"${s.value === c.son ? " selected" : ""}>${esc(s.label)}</option>`,
    )
    .join("");
  return `
    <header class="head row">
      <button class="back" id="back" aria-label="Retour">‹</button>
      <h1>Réglages de la dictée</h1>
    </header>
    <section class="group">
      <h2>Les trois boutons</h2>
      ${selects}
    </section>
    ${raccourcisSection(c)}
    <section class="group">
      <div class="field ligne-raccourci">
        <span>Mots à corriger${c.lexique.length ? ` (${c.lexique.length})` : ""}</span>
        <button class="touche" id="to-lexique">Modifier</button>
      </div>
      <p class="help">Quand la dictée écrit un mot de travers : un nom propre, une marque.</p>
    </section>
    <section class="group">
      <label class="field stacked">
        <span>Garder le micro ouvert après une dictée</span>
        <select id="warm">${warm}</select>
      </label>
      <p class="help">Tant que le micro reste ouvert, la dictée suivante démarre tout de suite et garde tes premiers mots. Un casque Bluetooth reste en mode appel pendant ce temps.</p>
    </section>
    <section class="group">
      <label class="field stacked">
        <span>Pendant que tu parles, le son du Mac</span>
        <select id="son">${son}</select>
      </label>
      <p class="help">Il revient à son niveau dès que tu relâches.</p>
    </section>
    <footer class="foot">
      <button class="link danger" id="quit">Quitter l'app</button>
      <button class="btn primary" id="done">Terminé</button>
    </footer>`;
}

function lexiqueView(c: Config): string {
  const liste = c.lexique.length
    ? `<ul class="mots">${c.lexique
        .map(
          (m) => `<li class="mot">
            <span class="de" title="${esc(m.de)}">${esc(m.de)}</span>
            <span class="fleche" aria-hidden="true">→</span>
            <span class="vers" title="${esc(m.vers)}">${esc(m.vers)}</span>
            <button class="retirer" data-de="${esc(m.de)}" title="Retirer" aria-label="Retirer ${esc(m.de)}">×</button>
          </li>`,
        )
        .join("")}</ul>`
    : `<p class="help">Aucune correction pour l'instant.</p>`;
  const erreur = erreurLexique ? `<p class="help erreur">${esc(erreurLexique)}</p>` : "";
  return `
    <header class="head row">
      <button class="back" id="back-lexique" aria-label="Retour">‹</button>
      <h1>Mots à corriger</h1>
    </header>
    <p class="help">Ce que la dictée écrit, et ce que tu veux à la place. Pris en compte dès la dictée suivante.</p>
    ${liste}
    <form class="group" id="ajout" novalidate>
      <h2>Ajouter</h2>
      <input id="de" placeholder="Elle écrit : Monsieur Guise" value="${esc(saisieDe)}" autocomplete="off" spellcheck="false" />
      <input id="vers" placeholder="Tu veux : Monsieur Guiz" value="${esc(saisieVers)}" autocomplete="off" spellcheck="false" />
      ${erreur}
      <button class="btn primary ajouter" type="submit">Ajouter</button>
    </form>
    <footer class="foot">
      <span></span>
      <button class="btn" id="done-lexique">Terminé</button>
    </footer>`;
}

async function ajouterCorrection() {
  if (!cfg) return;
  if (!saisieDe.trim() || !saisieVers.trim()) {
    erreurLexique = !saisieDe.trim() ? "Indique ce que la dictée écrit." : "Indique ce que tu veux à la place.";
    render();
    return;
  }
  try {
    cfg.lexique = await invoke<Correction[]>("ajouter_lexique", { de: saisieDe, vers: saisieVers });
    saisieDe = "";
    saisieVers = "";
    erreurLexique = "";
  } catch (e) {
    erreurLexique = String(e);
  }
  render();
  root.querySelector<HTMLInputElement>("#de")?.focus();
}

async function retirerCorrection(de: string) {
  if (!cfg) return;
  try {
    cfg.lexique = await invoke<Correction[]>("retirer_lexique", { de });
    erreurLexique = "";
  } catch (e) {
    erreurLexique = String(e);
  }
  render();
}

function render() {
  if (!cfg) return;
  root.innerHTML =
    view === "quick" ? quickView(cfg) : view === "setup" ? setupView(cfg) : lexiqueView(cfg);
  bind();
  requestAnimationFrame(fit);
}

// La hauteur du panneau suit son contenu (les deux vues diffèrent).
async function fit() {
  const height = Math.ceil(root.getBoundingClientRect().height);
  await invoke("fit_popover", { height });
}

async function choose(value: string) {
  if (!cfg) return;
  await invoke("set_dictation_target", { value });
  cfg.current = value;
  render();
  // Le temps de voir le bouton s'allumer, puis on rend la main à l'app d'avant.
  setTimeout(() => invoke("close_popover", { restoreFocus: true }), 180);
}

// --- saisie d'un raccourci ---
// Pendant la saisie, tous les raccourcis de l'app sont suspendus : la
// combinaison tapée ne doit ni lancer une dictée ni changer la langue.
async function debutSaisie(action: Action) {
  capture = action;
  controleSeul = false;
  erreurRaccourci = "";
  await invoke("suspendre_raccourcis", { actif: true });
  render();
}

function finSaisie() {
  if (!capture) return;
  capture = null;
  controleSeul = false;
  invoke("suspendre_raccourcis", { actif: false });
  render();
}

async function enregistrer(action: Action, valeur: string) {
  if (!cfg) return;
  try {
    cfg.raccourcis.actuels[action] = await invoke<string>("set_raccourci", { quoi: action, valeur });
    erreurRaccourci = "";
  } catch (e) {
    erreurRaccourci = String(e);
  }
  finSaisie();
}

document.addEventListener(
  "keydown",
  (e) => {
    if (!capture || !cfg) return;
    e.preventDefault();
    e.stopPropagation();
    const action = capture;
    const seul = !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey;
    if (e.key === "Escape") return finSaisie();
    if (e.key === "Backspace" && seul) return void enregistrer(action, cfg.raccourcis.defauts[action]);
    if (e.code === "ControlLeft" || e.code === "ControlRight") {
      // Contrôle seul : validé au relâchement, si aucune autre touche n'a suivi.
      controleSeul = !e.metaKey && !e.altKey && !e.shiftKey;
      return;
    }
    controleSeul = false;
    const combinaison = buildShortcut(e);
    if (combinaison) enregistrer(action, combinaison.accel);
  },
  true,
);

document.addEventListener(
  "keyup",
  (e) => {
    if (capture !== "dictee" || !controleSeul) return;
    if (e.code === "ControlLeft" || e.code === "ControlRight") enregistrer("dictee", "Control");
  },
  true,
);

// Panneau fermé en pleine saisie : on abandonne (Rust rétablit aussi les raccourcis).
window.addEventListener("blur", () => {
  if (capture) finSaisie();
});

function bind() {
  root.querySelector("#to-lexique")?.addEventListener("click", () => {
    finSaisie();
    view = "lexique";
    erreurLexique = "";
    render();
    root.querySelector<HTMLInputElement>("#de")?.focus();
  });
  const retourReglages = () => {
    view = "setup";
    render();
  };
  root.querySelector("#back-lexique")?.addEventListener("click", retourReglages);
  root.querySelector("#done-lexique")?.addEventListener("click", retourReglages);
  root.querySelector<HTMLInputElement>("#de")?.addEventListener("input", (e) => {
    saisieDe = (e.target as HTMLInputElement).value;
  });
  root.querySelector<HTMLInputElement>("#vers")?.addEventListener("input", (e) => {
    saisieVers = (e.target as HTMLInputElement).value;
  });
  root.querySelector<HTMLFormElement>("#ajout")?.addEventListener("submit", (e) => {
    e.preventDefault();
    ajouterCorrection();
  });
  root.querySelectorAll<HTMLButtonElement>(".retirer").forEach((b) =>
    b.addEventListener("click", () => retirerCorrection(b.dataset.de as string)),
  );
  root.querySelectorAll<HTMLButtonElement>("[data-raccourci]").forEach((b) =>
    b.addEventListener("click", () => {
      const action = b.dataset.raccourci as Action;
      if (capture === action) finSaisie();
      else debutSaisie(action);
    }),
  );
  root.querySelectorAll<HTMLButtonElement>(".tile").forEach((b) =>
    b.addEventListener("click", () => choose(b.dataset.value as string)),
  );
  root.querySelector("#to-setup")?.addEventListener("click", () => {
    view = "setup";
    render();
  });
  root.querySelector("#open-translator")?.addEventListener("click", () => invoke("open_translator"));
  root.querySelector("#back")?.addEventListener("click", () => {
    finSaisie();
    view = "quick";
    render();
  });
  root.querySelector("#done")?.addEventListener("click", () => {
    finSaisie();
    view = "quick";
    render();
  });
  root.querySelector("#quit")?.addEventListener("click", () => invoke("quit_app"));
  root.querySelectorAll<HTMLSelectElement>("select[data-i]").forEach((sel) =>
    sel.addEventListener("change", async () => {
      if (!cfg) return;
      cfg.quick[Number(sel.dataset.i)] = sel.value;
      await invoke("set_quick_targets", { values: cfg.quick });
    }),
  );
  root.querySelector<HTMLSelectElement>("#warm")?.addEventListener("change", async (e) => {
    if (!cfg) return;
    const secs = Number((e.target as HTMLSelectElement).value);
    cfg.warm_secs = secs;
    await invoke("set_mic_warm_secs", { secs });
  });
  root.querySelector<HTMLSelectElement>("#son")?.addEventListener("change", async (e) => {
    if (!cfg) return;
    const value = (e.target as HTMLSelectElement).value;
    cfg.son = value;
    await invoke("set_son_dictee", { value });
  });
}

// Clavier : Échap ferme, 1 / 2 / 3 choisissent un bouton.
document.addEventListener("keydown", (e) => {
  if (capture) return; // la saisie d'un raccourci a la main
  if (e.key === "Escape") {
    if (view === "lexique") {
      view = "setup";
      render();
    } else if (view === "setup") {
      view = "quick";
      render();
    } else {
      invoke("close_popover", { restoreFocus: true });
    }
    return;
  }
  if (view === "quick" && cfg && ["1", "2", "3"].includes(e.key)) {
    const value = cfg.quick[Number(e.key) - 1];
    if (value) choose(value);
  }
});

// À chaque ouverture : vue principale, réglages relus (ils ont pu changer).
listen("popover-shown", () => {
  view = "quick";
  capture = null;
  erreurRaccourci = "";
  load();
});

load();
