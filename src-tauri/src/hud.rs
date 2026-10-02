//! Petit indicateur temporaire, façon HUD système (volume, luminosité) :
//! affiche brièvement la langue choisie quand on la change au clavier
//! (raccourci + flèches), sans avoir à ouvrir le panneau de l'icône.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "hud";
const WIDTH: f64 = 200.0;
const HEIGHT: f64 = 96.0;
const VISIBLE_MS: u64 = 1100;

/// Incrémenté à chaque apparition : une fermeture programmée par une
/// apparition plus ancienne ne doit pas fermer une apparition plus récente.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Crée la fenêtre (cachée) au démarrage de l'app.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("hud.html".into()))
        .title("Langue de la dictée")
        .inner_size(WIDTH, HEIGHT)
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
                .effect(Effect::HudWindow)
                .state(EffectState::Active)
                .radius(16.0)
                .build(),
        )
        .build()?;
    // Ne capte jamais le clic : purement informatif, au-dessus de tout.
    let _ = win.set_ignore_cursor_events(true);
    Ok(())
}

/// Affiche la langue choisie environ une seconde, centrée en bas de l'écran
/// (comme les indicateurs système), puis se referme seule.
pub fn flash(app: &AppHandle, target: &crate::dictee::Target) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    // Injecté directement (pas d'évènement Tauri) : au tout premier appel
    // après le démarrage, un évènement peut arriver avant que la page ait
    // fini de s'enregistrer comme écouteur et se perdre silencieusement.
    // `eval` agit sur la page déjà chargée, sans cette course.
    let script = format!(
        "window.__hudSet && window.__hudSet({}, {})",
        serde_json::to_string(target.icon).unwrap_or_default(),
        serde_json::to_string(target.label).unwrap_or_default(),
    );
    let _ = win.eval(&script);

    let scale = win.scale_factor().unwrap_or(1.0);
    let (width, height) = (WIDTH * scale, HEIGHT * scale);
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let (mx, my) = (m.position().x as f64, m.position().y as f64);
        let (mw, mh) = (m.size().width as f64, m.size().height as f64);
        let x = mx + (mw - width) / 2.0;
        let y = my + mh - height - mh * 0.16;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
    let _ = win.show();

    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(VISIBLE_MS));
        // Une apparition plus récente a déjà pris le relais : ne rien fermer.
        if GENERATION.load(Ordering::SeqCst) == generation {
            if let Some(w) = app.get_webview_window(LABEL) {
                let _ = w.hide();
            }
        }
    });
}
