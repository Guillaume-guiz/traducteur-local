// Raccourcis clavier : affichage (« ⌘⇧T ») et saisie d'une combinaison au
// clavier, partagés par la fenêtre du traducteur et le panneau de la dictée.

// Symbole d'affichage pour une touche (hors modificateurs).
export function keyToDisplay(tok: string): string {
  const k = /^Key([A-Z])$/.exec(tok);
  if (k) return k[1];
  const d = /^Digit([0-9])$/.exec(tok);
  if (d) return d[1];
  const map: Record<string, string> = {
    Space: "␣",
    Enter: "⏎",
    Tab: "⇥",
    Backspace: "⌫",
    Delete: "⌦",
    Escape: "⎋",
    ArrowUp: "↑",
    ArrowDown: "↓",
    ArrowLeft: "←",
    ArrowRight: "→",
  };
  return map[tok] ?? tok;
}

// Accélérateur Tauri ("CmdOrCtrl+Shift+T") → affichage joli ("⌘⇧T").
export function accelToDisplay(accel: string): string {
  return accel
    .split("+")
    .map((tok) => {
      switch (tok) {
        case "CmdOrCtrl":
        case "Command":
        case "Cmd":
        case "Super":
        case "Meta":
          return "⌘";
        case "Control":
        case "Ctrl":
          return "⌃";
        case "Alt":
        case "Option":
          return "⌥";
        case "Shift":
          return "⇧";
        default:
          return keyToDisplay(tok);
      }
    })
    .join("");
}

// Code clavier (KeyboardEvent.code) → token accepté par l'accélérateur Tauri.
export function codeToAccel(code: string): string | null {
  const k = /^Key([A-Z])$/.exec(code);
  if (k) return k[1];
  const d = /^Digit([0-9])$/.exec(code);
  if (d) return d[1];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code; // F1..F24
  if (
    /^(Space|Enter|Tab|Backspace|Delete|Home|End|PageUp|PageDown|ArrowUp|ArrowDown|ArrowLeft|ArrowRight)$/.test(
      code,
    )
  ) {
    return code;
  }
  return null;
}

// Construit (accélérateur, affichage) à partir d'une frappe, ou null si incomplet.
export function buildShortcut(e: KeyboardEvent): { accel: string; display: string } | null {
  if (/^(Meta|Control|Alt|Shift)(Left|Right)$/.test(e.code)) return null; // modificateur seul
  const key = codeToAccel(e.code);
  if (!key) return null;
  const mods: string[] = [];
  const disp: string[] = [];
  if (e.metaKey) {
    mods.push("CmdOrCtrl");
    disp.push("⌘");
  }
  if (e.ctrlKey) {
    mods.push("Control");
    disp.push("⌃");
  }
  if (e.altKey) {
    mods.push("Alt");
    disp.push("⌥");
  }
  if (e.shiftKey) {
    mods.push("Shift");
    disp.push("⇧");
  }
  if (mods.length === 0) return null; // exige au moins un modificateur
  disp.push(keyToDisplay(key));
  return { accel: [...mods, key].join("+"), display: disp.join("") };
}
