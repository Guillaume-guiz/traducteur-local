//! Panneau de la barre de menus : un clic sur l'icône ouvre, sous l'icône, un
//! petit panneau avec trois boutons de langue pour la dictée et un bouton
//! Réglages (choix des trois boutons, durée pendant laquelle le micro reste
//! ouvert, son pendant la dictée). Remplace le sous-menu système, jugé peu
//! pratique (27/09/2026). S'ouvre aussi au clic droit sur la pastille.

use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Rect, WebviewUrl,
    WebviewWindowBuilder,
};

pub const LABEL: &str = "popover";
const WIDTH: f64 = 300.0;
/// Boutons proposés tant que l'utilisateur n'a rien choisi dans les réglages.
const DEFAULT_QUICK: [&str; 3] = ["none", "English", "Spanish"];
/// Choix proposés pour garder le micro ouvert après une dictée (secondes, -1 = toujours).
const WARM_CHOICES: [(i64, &str); 5] = [
    (0, "Non, le fermer tout de suite"),
    (30, "30 secondes"),
    (120, "2 minutes"),
    (600, "10 minutes"),
    (-1, "Toujours (le casque reste en mode appel)"),
];

/// Heure de la dernière fermeture par perte de focus : un clic sur l'icône qui
/// suit de près venait fermer le panneau, pas le rouvrir.
static LAST_BLUR_HIDE_MS: AtomicU64 = AtomicU64::new(0);
/// Bas du panneau quand il s'ouvre au-dessus de la pastille (pixels
/// physiques) : sa hauteur suit son contenu, il grandit alors vers le haut.
static BAS_AU_DESSUS: Mutex<Option<f64>> = Mutex::new(None);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Crée le panneau (caché) au démarrage de l'app.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popover.html".into()))
        .title("Dictée")
        .inner_size(WIDTH, 220.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        .visible_on_all_workspaces(true)
        .effects(
            EffectsBuilder::new()
                .effect(Effect::Popover)
                .state(EffectState::Active)
                .radius(12.0)
                .build(),
        )
        .build()?;
    Ok(())
}

/// Clic sur l'icône : ouvre le panneau sous l'icône, ou le ferme s'il est ouvert.
pub fn toggle(app: &AppHandle, icon: Rect) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    let pos: PhysicalPosition<f64> = icon.position.to_physical(scale);
    let size: tauri::PhysicalSize<f64> = icon.size.to_physical(scale);
    ouvrir_pres(app, pos.x, pos.y, size.width, size.height);
}

/// Ouvre le panneau sous un repère (icône, pastille), ou au-dessus s'il n'a pas
/// la place en dessous ; le ferme s'il est déjà ouvert. Coordonnées en pixels
/// physiques.
pub fn ouvrir_pres(app: &AppHandle, x0: f64, y0: f64, w0: f64, h0: f64) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
        return;
    }
    if now_ms().saturating_sub(LAST_BLUR_HIDE_MS.load(Ordering::SeqCst)) < 300 {
        return;
    }
    let scale = win.scale_factor().unwrap_or(1.0);
    let width = WIDTH * scale;
    let margin = 8.0 * scale;
    let gap = 4.0 * scale;
    let height = win.outer_size().map(|s| s.height as f64).unwrap_or(220.0 * scale);
    let mut x = x0 + w0 / 2.0 - width / 2.0;
    let mut y = y0 + h0 + gap;
    let mut bas = None;
    // Rester dans l'écran du repère (icône près du bord droit, pastille en bas).
    if let Ok(Some(m)) = app.monitor_from_point(x0 + w0 / 2.0, y0) {
        let (mx, my) = (m.position().x as f64, m.position().y as f64);
        let (mw, mh) = (m.size().width as f64, m.size().height as f64);
        x = x.min(mx + mw - width - margin).max(mx + margin);
        if y + height > my + mh - margin {
            bas = Some(y0 - gap);
            y = y0 - gap - height;
        }
    }
    *BAS_AU_DESSUS.lock().unwrap() = bas;
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.emit("popover-shown", ());
    // L'app a pu être masquée pour rendre la main à l'app d'avant : on la
    // réaffiche d'abord (seul le panneau est visible, la fenêtre du traducteur
    // reste fermée).
    let _ = app.show();
    let _ = win.show();
    let _ = win.set_focus();
}

/// Le panneau perd le focus (clic ailleurs) : on le ferme.
pub fn on_blur(window: &tauri::Window) {
    LAST_BLUR_HIDE_MS.store(now_ms(), Ordering::SeqCst);
    let _ = window.hide();
    // Saisie d'un raccourci interrompue : les raccourcis reviennent.
    crate::raccourcis::reprendre(window.app_handle());
}

/// Ferme le panneau ; `restore_focus` rend la main à l'app d'avant (celle où
/// l'on va dicter) si la fenêtre du traducteur n'est pas ouverte.
fn close(app: &AppHandle, restore_focus: bool) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
    }
    let main_visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if restore_focus && !main_visible {
        let _ = app.hide();
    }
}

fn quick_targets(app: &AppHandle) -> Vec<String> {
    let saved: Vec<String> = crate::read_settings(app)
        .get("dictation_quick")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .filter(|v| crate::dictee::is_known_target(v))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    if saved.len() == 3 {
        saved
    } else {
        DEFAULT_QUICK.iter().map(|s| s.to_string()).collect()
    }
}

#[derive(Serialize)]
pub struct TargetInfo {
    value: &'static str,
    label: &'static str,
    code: &'static str,
    icon: &'static str,
}

#[derive(Serialize)]
pub struct WarmChoice {
    secs: i64,
    label: &'static str,
}

#[derive(Serialize)]
pub struct SonChoice {
    value: &'static str,
    label: &'static str,
}

#[derive(Serialize)]
pub struct DictationConfig {
    current: String,
    quick: Vec<String>,
    targets: Vec<TargetInfo>,
    raccourcis: crate::raccourcis::Raccourcis,
    lexique: Vec<crate::lexique::Entree>,
    warm_secs: i64,
    warm_choices: Vec<WarmChoice>,
    son: String,
    son_choices: Vec<SonChoice>,
}

#[tauri::command]
pub fn get_dictation_config(app: AppHandle) -> DictationConfig {
    DictationConfig {
        current: crate::dictation_target(&app),
        quick: quick_targets(&app),
        targets: crate::dictee::TARGETS
            .iter()
            .map(|t| TargetInfo {
                value: t.value,
                label: t.label,
                code: t.code,
                icon: t.icon,
            })
            .collect(),
        raccourcis: crate::raccourcis::pour_panneau(&app),
        lexique: crate::lexique::lire(&app),
        warm_secs: crate::mic_warm_secs(&app),
        warm_choices: WARM_CHOICES
            .iter()
            .map(|(secs, label)| WarmChoice { secs: *secs, label })
            .collect(),
        son: crate::son::reglage(&app),
        son_choices: crate::son::CHOIX
            .iter()
            .map(|(value, label)| SonChoice { value, label })
            .collect(),
    }
}

/// Change la langue de la dictée d'un cran parmi les trois boutons rapides
/// (+1 suivante, -1 précédente) : raccourci clavier ou clic sur la langue de
/// la pastille. `hud` l'affiche brièvement au centre de l'écran (utile au
/// clavier ; la pastille affiche déjà la langue sous le curseur).
pub(crate) fn cycle_target(app: &AppHandle, delta: i32, hud: bool) {
    let quick = quick_targets(app);
    if quick.is_empty() {
        return;
    }
    let current = crate::dictation_target(app);
    let at = quick.iter().position(|v| *v == current).unwrap_or(0) as i32;
    let len = quick.len() as i32;
    let next = (((at + delta) % len) + len) % len;
    let value = quick[next as usize].clone();
    crate::save_setting(app, "dictation_target", &value);
    crate::pastille::langue(app);
    if !hud {
        return;
    }
    if let Some(t) = crate::dictee::TARGETS.iter().find(|t| t.value == value) {
        crate::hud::flash(app, t);
    }
}

#[tauri::command]
pub fn set_dictation_target(app: AppHandle, value: String) -> Result<(), String> {
    if !crate::dictee::is_known_target(&value) {
        return Err(format!("Langue inconnue : {value}"));
    }
    crate::save_setting(&app, "dictation_target", &value);
    crate::pastille::langue(&app);
    Ok(())
}

#[tauri::command]
pub fn set_quick_targets(app: AppHandle, values: Vec<String>) -> Result<(), String> {
    if values.len() != 3 || !values.iter().all(|v| crate::dictee::is_known_target(v)) {
        return Err("Il faut trois langues connues.".into());
    }
    let mut map = crate::read_settings(&app);
    map.insert(
        "dictation_quick".into(),
        Value::Array(values.into_iter().map(Value::String).collect()),
    );
    crate::write_settings(&app, &map);
    Ok(())
}

#[tauri::command]
pub fn set_mic_warm_secs(app: AppHandle, secs: i64) -> Result<(), String> {
    if !WARM_CHOICES.iter().any(|(s, _)| *s == secs) {
        return Err("Durée non proposée.".into());
    }
    let mut map = crate::read_settings(&app);
    map.insert("mic_warm_secs".into(), Value::from(secs));
    crate::write_settings(&app, &map);
    Ok(())
}

#[tauri::command]
pub fn set_son_dictee(app: AppHandle, value: String) -> Result<(), String> {
    if !crate::son::CHOIX.iter().any(|(v, _)| *v == value) {
        return Err("Choix non proposé.".into());
    }
    crate::save_setting(&app, "son_dictee", &value);
    Ok(())
}

#[tauri::command]
pub fn close_popover(app: AppHandle, restore_focus: bool) {
    close(&app, restore_focus);
}

/// Ajuste la hauteur du panneau à son contenu (les deux vues n'ont pas la même taille).
#[tauri::command]
pub fn fit_popover(app: AppHandle, height: f64) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let height = height.clamp(80.0, 760.0);
        let _ = win.set_size(LogicalSize::new(WIDTH, height));
        // Ouvert au-dessus de la pastille : le bas reste collé à la pastille.
        if let Some(bas) = *BAS_AU_DESSUS.lock().unwrap() {
            if let Ok(pos) = win.outer_position() {
                let scale = win.scale_factor().unwrap_or(1.0);
                let _ = win.set_position(PhysicalPosition::new(pos.x as f64, bas - height * scale));
            }
        }
    }
}

#[tauri::command]
pub fn open_translator(app: AppHandle) {
    close(&app, false);
    crate::show_main_window(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
