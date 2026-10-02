// Pastille flottante : l'état de la dictée en un coup d'œil. Rust pousse les
// changements en appelant directement window.__pastille, pas par un évènement
// Tauri : un évènement envoyé avant que la page écoute se perdrait sans bruit
// (vu sur l'indicateur de langue le 27/09/2026).
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

type Langue = { code: string; label: string };
type Message = {
  type: "etat" | "niveau" | "langue";
  etat?: string;
  brut?: boolean;
  parole?: number;
  texte?: string;
  v?: number;
  code?: string;
  label?: string;
};

const el = document.getElementById("pastille") as HTMLElement;

const svg = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${d}"/></svg>`;
const ICONES = {
  agrandir: svg("M15 4h5v5M9 20H4v-5M20 4l-6 6M4 20l6-6"),
  coche: svg("M5 12l5 5L20 7"),
  alerte: svg("M12 9v4M12 17h.01M10.3 3.9L2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"),
};
// Hauteur relative des cinq barres de la voix : plus hautes au centre.
const FORME = [0.55, 0.85, 1, 0.8, 0.6];

let etat = "repos";
let langue: Langue = { code: "", label: "" };
let niv = 0;
let debutEcoute = 0;
let generation = 0;
let chrono: number | undefined;
let retour: number | undefined;
let largeurEnvoyee = 0;

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string);

function boutonLangue(): string {
  const titre = `${langue.label} : clic pour changer, clic droit pour le panneau`;
  return `<button class="langue" data-act="langue" title="${esc(titre)}">${esc(langue.code)}</button>`;
}

function contenu(m?: Message): string {
  switch (etat) {
    case "micro":
      return `<span class="point attente"></span><span class="texte doux">Micro... attends</span>${boutonLangue()}`;
    case "ecoute":
      return `<span class="point rouge"></span><span class="barres">${"<i></i>".repeat(5)}</span><span class="chrono">0:00</span>${boutonLangue()}`;
    case "traitement":
      return `<span class="roue"></span><span class="texte">${m?.brut ? "Transcription" : "Traduction"}</span><span class="progression"><i></i></span>${boutonLangue()}`;
    case "colle":
      return `${ICONES.coche}<span class="texte">Collé</span>${boutonLangue()}`;
    case "erreur":
      return `${ICONES.alerte}<span class="texte">${esc(m?.texte ?? "Échec dictée")}</span>`;
    default:
      return `<span class="point"></span>${boutonLangue()}<button class="agrandir" data-act="agrandir" title="Ouvrir le traducteur" aria-label="Ouvrir le traducteur">${ICONES.agrandir}</button>`;
  }
}

function afficher(nouvel: string, m?: Message) {
  etat = nouvel;
  generation++;
  window.clearInterval(chrono);
  window.clearTimeout(retour);
  el.className = `pastille ${etat}`;
  el.innerHTML = contenu(m);
  niv = 0;
  el.style.setProperty("--niv", "0");
  const g = generation;
  if (etat === "ecoute") {
    debutEcoute = Date.now();
    chrono = window.setInterval(majChrono, 250);
  } else if (etat === "traitement") {
    lancerProgression(m);
  } else if (etat === "colle") {
    retour = window.setTimeout(() => g === generation && afficher("repos"), 1300);
  } else if (etat === "erreur") {
    // Rust ramène au repos après 4 s ; filet de sécurité si ce retour se perd.
    retour = window.setTimeout(() => g === generation && afficher("repos"), 6000);
  }
  ajuster();
}

function majChrono() {
  const s = Math.floor((Date.now() - debutEcoute) / 1000);
  const t = el.querySelector(".chrono");
  if (t) t.textContent = `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

// Barre d'attente : durée estimée d'après le temps de parole (journal du
// Mac Studio : transcription environ 0,06 s par seconde de parole, traduction
// environ 0,13 s). Elle s'arrête à 92 % en attendant le vrai résultat.
function lancerProgression(m?: Message) {
  const s = m?.parole ?? 3;
  const ms = m?.brut ? 100 + 57 * s : 400 + 135 * s;
  const barre = el.querySelector<HTMLElement>(".progression i");
  if (!barre) return;
  barre.style.transition = `width ${Math.round(ms)}ms cubic-bezier(.2,.6,.4,1)`;
  requestAnimationFrame(() => requestAnimationFrame(() => (barre.style.width = "92%")));
}

// Niveau de la voix (RMS brut) : un micro au repos tourne autour de 0,002,
// une voix normale entre 0,02 et 0,1. Montée rapide, descente plus douce.
function niveau(v: number) {
  if (etat !== "ecoute") return;
  const cible = Math.max(0, Math.min(1, (Math.sqrt(Math.max(0, v)) - 0.05) * 3.6));
  niv = cible > niv ? niv * 0.35 + cible * 0.65 : niv * 0.75 + cible * 0.25;
  el.style.setProperty("--niv", niv.toFixed(3));
  el.querySelectorAll<HTMLElement>(".barres i").forEach((b, i) => {
    const h = 3 + Math.min(1, niv * FORME[i] * (0.8 + Math.random() * 0.4)) * 13;
    b.style.height = `${h.toFixed(1)}px`;
  });
}

// La fenêtre prend la largeur de la pastille, plus la marge du halo.
function ajuster() {
  requestAnimationFrame(() => {
    const largeur = Math.ceil(el.getBoundingClientRect().width) + 32;
    if (Math.abs(largeur - largeurEnvoyee) < 1) return;
    largeurEnvoyee = largeur;
    invoke("pastille_ajuster", { largeur });
  });
}

function majLangue() {
  const b = el.querySelector<HTMLElement>(".langue");
  if (!b) return;
  b.textContent = langue.code;
  b.title = `${langue.label} : clic pour changer, clic droit pour le panneau`;
  ajuster();
}

(window as unknown as { __pastille: (m: Message) => void }).__pastille = (m) => {
  if (m.type === "niveau") niveau(m.v ?? 0);
  else if (m.type === "langue") {
    langue = { code: m.code ?? "", label: m.label ?? "" };
    majLangue();
  } else if (m.type === "etat" && m.etat) afficher(m.etat, m);
};

// Glisser la pastille depuis n'importe où, boutons compris (30/09/2026, retour
// de Guillaume) : un appui suivi d'un mouvement de plus de 4 px déplace la
// fenêtre ; un appui relâché sans bouger déclenche le bouton sous la souris.
let appui: { x: number; y: number; bouton: HTMLElement | null } | null = null;

function agir(bouton: HTMLElement | null) {
  if (bouton?.dataset.act === "langue") invoke("pastille_langue_suivante");
  else if (bouton?.dataset.act === "agrandir") invoke("pastille_agrandir");
}

el.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  appui = { x: e.clientX, y: e.clientY, bouton: (e.target as HTMLElement).closest<HTMLElement>("[data-act]") };
});

document.addEventListener("mousemove", (e) => {
  if (!appui) return;
  if (Math.abs(e.clientX - appui.x) + Math.abs(e.clientY - appui.y) > 4) {
    appui = null;
    invoke("pastille_glisser");
    getCurrentWindow().startDragging();
  }
});

document.addEventListener("mouseup", (e) => {
  if (!appui || e.button !== 0) return;
  const { bouton } = appui;
  appui = null;
  agir(bouton);
});

// Un clic ne fait rien de plus : tout passe par l'appui et le relâchement.
el.addEventListener("click", (e) => e.preventDefault());

// Clic droit : le panneau de la dictée (langues, réglages), contre la pastille.
document.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  invoke("pastille_panneau");
});

afficher("repos");
invoke<{ langue: Langue }>("pastille_etat").then((r) => {
  langue = r.langue;
  majLangue();
});
