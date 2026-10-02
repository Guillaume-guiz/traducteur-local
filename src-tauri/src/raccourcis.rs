//! Raccourcis clavier réglables depuis le panneau de la dictée (29/09/2026) :
//! dicter (touche maintenue), langue suivante, langue précédente, traduire la
//! sélection. Pendant qu'on en tape un nouveau, tous sont suspendus, pour que
//! la combinaison tapée ne déclenche rien (ni dictée, ni changement de langue).

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

pub const DEFAUT_SUIVANTE: &str = "Control+Alt+ArrowRight";
pub const DEFAUT_PRECEDENTE: &str = "Control+Alt+ArrowLeft";
const CLE_SUIVANTE: &str = "raccourci_langue_suivante";
const CLE_PRECEDENTE: &str = "raccourci_langue_precedente";

/// Vrai pendant qu'on tape un nouveau raccourci dans le panneau.
static SUSPENDUS: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Clone)]
pub struct Jeu {
    dictee: String,
    suivante: String,
    precedente: String,
    traduction: String,
}

#[derive(Serialize)]
pub struct Raccourcis {
    actuels: Jeu,
    defauts: Jeu,
}

fn lu(app: &AppHandle, cle: &str, defaut: &str) -> String {
    crate::read_settings(app)
        .get(cle)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(defaut)
        .to_string()
}

pub fn suivante_enregistree(app: &AppHandle) -> String {
    lu(app, CLE_SUIVANTE, DEFAUT_SUIVANTE)
}

pub fn precedente_enregistree(app: &AppHandle) -> String {
    lu(app, CLE_PRECEDENTE, DEFAUT_PRECEDENTE)
}

fn actuels(app: &AppHandle) -> Jeu {
    let st = app.state::<crate::AppState>();
    let dictee = app.state::<crate::dictee::DictationState>();
    let lire = |m: &std::sync::Mutex<String>| m.lock().map(|g| g.clone()).unwrap_or_default();
    Jeu {
        dictee: lire(&dictee.shortcut),
        suivante: lire(&st.suivante),
        precedente: lire(&st.precedente),
        traduction: lire(&st.shortcut),
    }
}

/// Raccourcis actuels et d'origine, pour le panneau.
pub fn pour_panneau(app: &AppHandle) -> Raccourcis {
    Raccourcis {
        actuels: actuels(app),
        defauts: Jeu {
            dictee: crate::dictee::DEFAULT_DICTATION_SHORTCUT.into(),
            suivante: DEFAUT_SUIVANTE.into(),
            precedente: DEFAUT_PRECEDENTE.into(),
            traduction: crate::DEFAULT_SHORTCUT.into(),
        },
    }
}

/// Deux écritures du même raccourci (« Alt+Control+D » et « Control+Alt+D »).
fn meme(a: &str, b: &str) -> bool {
    match (a.parse::<Shortcut>(), b.parse::<Shortcut>()) {
        (Ok(x), Ok(y)) => x.id() == y.id(),
        _ => a.eq_ignore_ascii_case(b),
    }
}

/// Raccourcis enregistrés auprès de macOS (la dictée au Contrôle seul passe par
/// l'écoute du clavier, pas par un raccourci système).
fn combinaisons(jeu: &Jeu) -> Vec<String> {
    let mut v = vec![jeu.traduction.clone(), jeu.suivante.clone(), jeu.precedente.clone()];
    if !crate::dictee::is_control_mode(&jeu.dictee) {
        v.push(jeu.dictee.clone());
    }
    v
}

fn nom(quoi: &str) -> &'static str {
    match quoi {
        "dictee" => "dicter",
        "suivante" => "langue suivante",
        "precedente" => "langue précédente",
        _ => "traduire la sélection",
    }
}

/// Remplace un raccourci système : enregistre le nouveau, retire l'ancien ;
/// si macOS refuse le nouveau, l'ancien reste.
fn remplacer(
    app: &AppHandle,
    slot: &std::sync::Mutex<String>,
    cle: &str,
    valeur: &str,
) -> Result<String, String> {
    let gs = app.global_shortcut();
    let mut cur = slot.lock().map_err(|_| "État interne indisponible.".to_string())?;
    let _ = gs.unregister(cur.as_str());
    if let Err(e) = gs.register(valeur) {
        let _ = gs.register(cur.as_str());
        return Err(format!("macOS refuse cette combinaison ({e}). Essaie une autre."));
    }
    *cur = valeur.to_string();
    crate::save_setting(app, cle, valeur);
    Ok(valeur.to_string())
}

/// Change la touche de dictée : Contrôle seul (écoute du clavier) ou une
/// combinaison maintenue (raccourci système).
fn changer_dictee(app: &AppHandle, valeur: &str) -> Result<String, String> {
    let state = app.state::<crate::dictee::DictationState>();
    let mut cur = state
        .shortcut
        .lock()
        .map_err(|_| "État interne indisponible.".to_string())?;
    let gs = app.global_shortcut();
    let controle = crate::dictee::is_control_mode(valeur);
    if !controle {
        gs.register(valeur)
            .map_err(|e| format!("macOS refuse cette combinaison ({e}). Essaie une autre."))?;
    }
    if !crate::dictee::is_control_mode(&cur) {
        let _ = gs.unregister(cur.as_str());
    }
    *cur = valeur.to_string();
    crate::save_setting(app, "dictation_shortcut", valeur);
    crate::controle::activer(app, controle);
    Ok(valeur.to_string())
}

/// Change un raccourci (`quoi` : dictee, suivante, precedente, traduction).
#[tauri::command]
pub fn set_raccourci(app: AppHandle, quoi: String, valeur: String) -> Result<String, String> {
    let valeur = valeur.trim().to_string();
    if valeur.is_empty() {
        return Err("Raccourci vide.".into());
    }
    let controle = crate::dictee::is_control_mode(&valeur);
    if controle && quoi != "dictee" {
        return Err("Contrôle seul ne sert qu'à dicter.".into());
    }
    if !controle && valeur.parse::<Shortcut>().is_err() {
        return Err("Combinaison non reconnue.".into());
    }
    let jeu = actuels(&app);
    let autres = [
        ("dictee", &jeu.dictee),
        ("suivante", &jeu.suivante),
        ("precedente", &jeu.precedente),
        ("traduction", &jeu.traduction),
    ];
    if let Some((autre, _)) = autres.iter().find(|(q, v)| *q != quoi && meme(v, &valeur)) {
        return Err(format!("Déjà pris pour « {} ».", nom(autre)));
    }
    let st = app.state::<crate::AppState>();
    match quoi.as_str() {
        "dictee" => changer_dictee(&app, &valeur),
        "suivante" => remplacer(&app, &st.suivante, CLE_SUIVANTE, &valeur),
        "precedente" => remplacer(&app, &st.precedente, CLE_PRECEDENTE, &valeur),
        "traduction" => {
            let r = remplacer(&app, &st.shortcut, "shortcut", &valeur)?;
            // La fenêtre du traducteur affiche ce raccourci dans son pied de page.
            let _ = app.emit_to("main", "raccourci-traduction", &r);
            Ok(r)
        }
        _ => Err("Raccourci inconnu.".into()),
    }
}

/// Suspend (`actif`) ou rétablit tous les raccourcis de l'app.
#[tauri::command]
pub fn suspendre_raccourcis(app: AppHandle, actif: bool) {
    if actif {
        suspendre(&app);
    } else {
        reprendre(&app);
    }
}

fn suspendre(app: &AppHandle) {
    SUSPENDUS.store(true, Ordering::SeqCst);
    crate::controle::suspendre(true);
    let gs = app.global_shortcut();
    for c in combinaisons(&actuels(app)) {
        let _ = gs.unregister(c.as_str());
    }
}

/// Rétablit les raccourcis s'ils étaient suspendus (aussi appelé quand le
/// panneau se ferme, au cas où la saisie n'aurait pas été terminée).
pub fn reprendre(app: &AppHandle) {
    if !SUSPENDUS.swap(false, Ordering::SeqCst) {
        return;
    }
    let gs = app.global_shortcut();
    for c in combinaisons(&actuels(app)) {
        // Erreur sans conséquence si le raccourci est déjà actif.
        let _ = gs.register(c.as_str());
    }
    crate::controle::suspendre(false);
}
