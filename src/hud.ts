// Petit indicateur temporaire : affiche la langue choisie au clavier
// (raccourci + flèches), sans ouvrir le panneau de l'icône. Rust pilote tout
// (affichage, position, fermeture) et appelle directement window.__hudSet
// plutôt qu'un évènement Tauri : au tout premier changement de langue après
// le démarrage, un évènement peut arriver avant que cette page ait fini de
// s'enregistrer comme écouteur, et se perd alors silencieusement.

const iconEl = document.getElementById("icon") as HTMLElement;
const labelEl = document.getElementById("label") as HTMLElement;

(window as unknown as { __hudSet: (icon: string, label: string) => void }).__hudSet = (icon, label) => {
  iconEl.textContent = icon;
  labelEl.textContent = label;
};
