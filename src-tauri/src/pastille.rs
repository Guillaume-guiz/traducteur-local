//! Pastille flottante : montre à tout moment l'état de la dictée (repos,
//! micro en préparation, écoute avec le niveau de la voix, transcription ou
//! traduction, collé, erreur). Toujours au-dessus des fenêtres, déplaçable à
//! la souris, sa place est retenue. Un clic sur la langue la change, l'icône
//! d'agrandissement ouvre le traducteur avec la dernière dictée, un clic
//! droit ouvre le panneau de la dictée (langues et réglages), comme l'icône
//! de la barre de menus, invisible sur un écran à encoche chargé (29/09/2026).
//!
//! Elle ne prend jamais la main : l'app où l'on écrit reste active, sinon le
//! texte dicté se collerait dans la pastille au lieu du champ visé.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const LABEL: &str = "pastille";
/// Hauteur de la fenêtre : la pastille (30) plus la marge du halo lumineux.
const HEIGHT: f64 = 54.0;
/// Largeur de départ, corrigée par la page dès son premier affichage.
const WIDTH_INIT: f64 = 110.0;
/// Distance au bas de l'écran par défaut : au-dessus du Dock.
const DEFAULT_BOTTOM: f64 = 110.0;

/// Centre horizontal et haut de la pastille, en pixels physiques : elle
/// s'élargit et se rétrécit autour de son centre.
static ANCRE: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// Dernière position donnée par l'app : un déplacement vers cette position
/// n'est pas un glisser de l'utilisateur.
static POSEE: Mutex<Option<(i32, i32)>> = Mutex::new(None);
/// Incrémenté à chaque glisser : seul le dernier enregistre la position.
static GLISSER: AtomicU64 = AtomicU64::new(0);
/// Vrai pendant qu'on glisse la pastille à la souris. Sans ce drapeau, les
/// déplacements faits par l'app elle-même (création de la fenêtre, ajustement
/// de largeur) étaient pris pour un glisser et écrasaient la place retenue :
/// la pastille se retrouvait en bas à gauche au lancement suivant (30/09/2026).
static GLISSER_EN_COURS: AtomicBool = AtomicBool::new(false);

/// Un état de la dictée, tel que la pastille l'affiche.
pub enum Etat<'a> {
    Repos,
    /// Micro en cours d'ouverture (un casque Bluetooth peut prendre 1 à 3 s).
    Micro,
    Ecoute,
    Traitement { brut: bool, parole: f32 },
    Colle,
    Erreur(&'a str),
}

/// Crée la pastille au démarrage de l'app, visible, à sa dernière place.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("pastille.html".into()))
        .title("Dictée")
        .inner_size(WIDTH_INIT, HEIGHT)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .visible_on_all_workspaces(true)
        .build()?;
    sans_prise_de_main(&win);
    *ANCRE.lock().unwrap() = Some(ancre_initiale(app));
    placer(&win, WIDTH_INIT);
    let _ = win.show();
    Ok(())
}

/// Réglages macOS que Tauri n'expose pas : un clic sur la pastille n'active
/// pas l'app (l'app où l'on écrit garde la main), la pastille reste affichée
/// quand l'app se masque pour rendre la main (panneau, bouton Insérer), et
/// elle passe aussi par-dessus une app en plein écran.
#[cfg(target_os = "macos")]
fn sans_prise_de_main(win: &WebviewWindow) {
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, sel};
    let Ok(ptr) = win.ns_window() else { return };
    let ns = unsafe { &*(ptr as *mut AnyObject) };
    unsafe {
        let _: () = msg_send![ns, setCanHide: false];
        // Réglage interne d'AppKit, présent depuis des années mais non documenté :
        // on vérifie qu'il existe avant de s'en servir.
        let sans_activation: bool = msg_send![ns, respondsToSelector: sel!(_setPreventsActivation:)];
        if sans_activation {
            let _: () = msg_send![ns, _setPreventsActivation: true];
        }
        // Rejoint les apps en plein écran (1 << 8) et reste hors du cycle ⌘` (1 << 6).
        let comportement: usize = msg_send![ns, collectionBehavior];
        let _: () = msg_send![ns, setCollectionBehavior: comportement | (1 << 8) | (1 << 6)];
    }
}

#[cfg(not(target_os = "macos"))]
fn sans_prise_de_main(_win: &WebviewWindow) {}

/// Dernière place retenue si elle est encore sur un écran branché, sinon en
/// bas au centre de l'écran principal.
fn ancre_initiale(app: &AppHandle) -> (f64, f64) {
    let s = crate::read_settings(app);
    let x = s.get("pastille_x").and_then(|v| v.as_f64());
    let y = s.get("pastille_y").and_then(|v| v.as_f64());
    if let (Some(x), Some(y)) = (x, y) {
        if app.monitor_from_point(x, y + 1.0).ok().flatten().is_some() {
            return (x, y);
        }
    }
    match app.primary_monitor().ok().flatten() {
        Some(m) => {
            let (mx, my) = (m.position().x as f64, m.position().y as f64);
            let (mw, mh) = (m.size().width as f64, m.size().height as f64);
            (mx + mw / 2.0, my + mh - DEFAULT_BOTTOM * m.scale_factor())
        }
        None => (800.0, 800.0),
    }
}

/// Donne à la fenêtre la largeur voulue, centrée sur l'ancre, sans sortir de
/// l'écran.
fn placer(win: &WebviewWindow, largeur: f64) {
    let Some((cx, top)) = *ANCRE.lock().unwrap() else { return };
    let scale = win.scale_factor().unwrap_or(1.0);
    let (w, h) = (largeur * scale, HEIGHT * scale);
    let (mut x, mut y) = (cx - w / 2.0, top);
    if let Ok(Some(m)) = win.app_handle().monitor_from_point(cx, top + 1.0) {
        let (mx, my) = (m.position().x as f64, m.position().y as f64);
        let (mw, mh) = (m.size().width as f64, m.size().height as f64);
        x = x.min(mx + mw - w).max(mx);
        y = y.min(my + mh - h).max(my);
    }
    let (xi, yi) = (x.round() as i32, y.round() as i32);
    *POSEE.lock().unwrap() = Some((xi, yi));
    let _ = win.set_size(LogicalSize::new(largeur, HEIGHT));
    let _ = win.set_position(PhysicalPosition::new(xi, yi));
}

/// La fenêtre a bougé : si c'est l'utilisateur qui l'a glissée, on retient la
/// nouvelle place (enregistrée une fois le geste fini).
pub fn deplacee(window: &tauri::Window, pos: &PhysicalPosition<i32>) {
    if !GLISSER_EN_COURS.load(Ordering::SeqCst) {
        return;
    }
    if let Some((x, y)) = *POSEE.lock().unwrap() {
        if (x - pos.x).abs() <= 2 && (y - pos.y).abs() <= 2 {
            return;
        }
    }
    let largeur = window.outer_size().map(|s| s.width as f64).unwrap_or(0.0);
    let ancre = (pos.x as f64 + largeur / 2.0, pos.y as f64);
    *ANCRE.lock().unwrap() = Some(ancre);
    let gen = GLISSER.fetch_add(1, Ordering::SeqCst) + 1;
    let app = window.app_handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        if GLISSER.load(Ordering::SeqCst) != gen {
            return;
        }
        GLISSER_EN_COURS.store(false, Ordering::SeqCst);
        let mut map = crate::read_settings(&app);
        map.insert("pastille_x".into(), Value::from(ancre.0));
        map.insert("pastille_y".into(), Value::from(ancre.1));
        crate::write_settings(&app, &map);
    });
}

/// Code et nom de la langue de sortie de la dictée.
fn infos_langue(app: &AppHandle) -> Value {
    let cible = crate::dictation_target(app);
    match crate::dictee::TARGETS.iter().find(|t| t.value == cible) {
        Some(t) => json!({ "code": t.code, "label": t.label }),
        None => json!({ "code": "?", "label": cible }),
    }
}

/// Appelle directement la page (pas d'évènement Tauri) : un évènement envoyé
/// avant que la page écoute se perdrait sans bruit, comme vu sur l'indicateur
/// de langue le 27/09/2026.
fn envoyer(app: &AppHandle, msg: &Value) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.eval(&format!("window.__pastille && window.__pastille({msg})"));
    }
}

pub fn etat(app: &AppHandle, e: Etat) {
    let msg = match e {
        Etat::Repos => json!({ "type": "etat", "etat": "repos" }),
        Etat::Micro => json!({ "type": "etat", "etat": "micro" }),
        Etat::Ecoute => json!({ "type": "etat", "etat": "ecoute" }),
        Etat::Traitement { brut, parole } => {
            json!({ "type": "etat", "etat": "traitement", "brut": brut, "parole": parole })
        }
        Etat::Colle => json!({ "type": "etat", "etat": "colle" }),
        Etat::Erreur(texte) => json!({ "type": "etat", "etat": "erreur", "texte": texte }),
    };
    envoyer(app, &msg);
}

/// Niveau sonore du micro pendant l'écoute (RMS brut, mis en forme par la page).
pub fn niveau(app: &AppHandle, rms: f32) {
    envoyer(app, &json!({ "type": "niveau", "v": (rms * 10_000.0).round() / 10_000.0 }));
}

/// La langue de la dictée a changé (panneau, clavier, pastille).
pub fn langue(app: &AppHandle) {
    let mut msg = infos_langue(app);
    msg["type"] = json!("langue");
    envoyer(app, &msg);
}

/// Au chargement de la page : langue affichée.
#[tauri::command]
pub fn pastille_etat(app: AppHandle) -> Value {
    json!({ "langue": infos_langue(&app) })
}

/// La page commence un glisser à la souris : les déplacements qui suivent
/// sont les siens.
#[tauri::command]
pub fn pastille_glisser() {
    GLISSER_EN_COURS.store(true, Ordering::SeqCst);
}

/// La page a mesuré la pastille : la fenêtre prend cette largeur.
#[tauri::command]
pub fn pastille_ajuster(app: AppHandle, largeur: f64) {
    if let Some(win) = app.get_webview_window(LABEL) {
        placer(&win, largeur.clamp(60.0, 640.0));
    }
}

/// Clic sur la langue : langue suivante parmi les trois boutons du panneau.
#[tauri::command]
pub fn pastille_langue_suivante(app: AppHandle) {
    crate::popover::cycle_target(&app, 1, false);
}

/// Icône d'agrandissement : ouvre le traducteur, rempli avec la dernière
/// dictée si elle n'y a pas déjà été versée.
#[tauri::command]
pub fn pastille_agrandir(app: AppHandle) {
    let derniere = app
        .state::<crate::dictee::DictationState>()
        .derniere
        .lock()
        .ok()
        .and_then(|mut g| g.take());
    crate::show_main_window(&app);
    if let Some(d) = derniere {
        let _ = app.emit_to(
            "main",
            "derniere-dictee",
            json!({ "dit": d.dit, "texte": d.texte, "cible": d.cible }),
        );
    }
}

/// Clic droit : le panneau de la dictée, contre la pastille.
#[tauri::command]
pub fn pastille_panneau(app: AppHandle) {
    let Some(w) = app.get_webview_window(LABEL) else { return };
    let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size()) else { return };
    crate::popover::ouvrir_pres(
        &app,
        pos.x as f64,
        pos.y as f64,
        size.width as f64,
        size.height as f64,
    );
}
