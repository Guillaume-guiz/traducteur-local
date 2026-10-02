//! Baisse du son pendant la dictée : la musique ou la vidéo passe au quart de
//! son volume tant qu'on parle, puis revient au relâchement (demande de
//! Guillaume du 29/09/2026). On baisse plutôt que mettre en pause : la touche
//! lecture/pause relancerait une musique qui était arrêtée.
//!
//! Le 27/09/2026, Superwhisper avait laissé le volume du Mac à 0. Ici, le
//! niveau d'avant est noté dans les réglages avant de baisser, et toujours
//! rendu : fin de dictée, abandon, micro indisponible, fermeture de l'app, et
//! au démarrage suivant si l'app s'est arrêtée en pleine dictée.

use serde_json::Value;
use std::sync::mpsc;
use std::sync::OnceLock;
use tauri::AppHandle;

/// Valeurs du réglage `son_dictee`, avec leur libellé dans le panneau.
pub const CHOIX: [(&str, &str); 3] = [
    ("baisser", "baisse au quart"),
    ("couper", "se coupe"),
    ("garder", "ne change pas"),
];
/// Clé des réglages où l'on note le volume d'avant la baisse.
const NOTE: &str = "son_avant_baisse";

enum Ordre {
    Baisser,
    Remonter,
}

static ORDRES: OnceLock<mpsc::Sender<Ordre>> = OnceLock::new();

pub fn reglage(app: &AppHandle) -> String {
    crate::read_settings(app)
        .get("son_dictee")
        .and_then(|v| v.as_str())
        .filter(|v| CHOIX.iter().any(|(c, _)| c == v))
        .unwrap_or("baisser")
        .to_string()
}

/// Démarre le fil qui applique baisses et remontées dans l'ordre où elles
/// arrivent : une remontée ne peut pas passer avant la baisse qui la précède.
pub fn demarrer(app: &AppHandle) {
    let (tx, rx) = mpsc::channel::<Ordre>();
    if ORDRES.set(tx).is_err() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // Baisse restée en plan au dernier arrêt de l'app : on rend le son.
        remonter_maintenant(&app);
        let mut baisse = false;
        for ordre in rx {
            match ordre {
                Ordre::Baisser if !baisse => baisse = baisser(&app),
                Ordre::Remonter if baisse => {
                    remonter_maintenant(&app);
                    baisse = false;
                }
                _ => {}
            }
        }
    });
}

/// Début de l'écoute : baisse le son, selon le réglage.
pub fn baisser_pendant_la_dictee(app: &AppHandle) {
    if reglage(app) == "garder" {
        return;
    }
    if let Some(tx) = ORDRES.get() {
        let _ = tx.send(Ordre::Baisser);
    }
}

/// Fin de l'écoute : rend le son (sans effet s'il n'a pas été baissé).
pub fn remonter() {
    if let Some(tx) = ORDRES.get() {
        let _ = tx.send(Ordre::Remonter);
    }
}

/// Rend tout de suite le volume noté, s'il y en a un (démarrage, fermeture).
pub fn remonter_maintenant(app: &AppHandle) {
    let Some(avant) = crate::read_settings(app).get(NOTE).and_then(|v| v.as_i64()) else {
        return;
    };
    if osascript(&format!("set volume output volume {avant}")).is_some() {
        let mut map = crate::read_settings(app);
        map.remove(NOTE);
        crate::write_settings(app, &map);
        crate::dictee::log_line(app, &format!("son_remonte volume={avant}"));
    } else {
        // La note reste : nouvel essai à la prochaine remontée ou au démarrage.
        crate::dictee::log_line(app, "son_remonte echec");
    }
}

/// Lit le volume, le note, puis le baisse. Renvoie vrai si le son a baissé.
fn baisser(app: &AppHandle) -> bool {
    let lu = osascript(
        "set s to get volume settings\n\
         if output muted of s then return -1\n\
         set v to output volume of s\n\
         if v is missing value then return -1\n\
         return v",
    );
    // -1 : son déjà coupé, ou sortie sans réglage de volume (certains écrans).
    let Some(avant) = lu.and_then(|s| s.parse::<i64>().ok()).filter(|v| *v > 0) else {
        return false;
    };
    let pendant = if reglage(app) == "couper" { 0 } else { avant / 4 };
    if pendant >= avant {
        return false;
    }
    // Noté avant de toucher au volume : même un arrêt brutal le laisse retrouvable.
    let mut map = crate::read_settings(app);
    map.insert(NOTE.into(), Value::from(avant));
    crate::write_settings(app, &map);
    if osascript(&format!("set volume output volume {pendant}")).is_none() {
        remonter_maintenant(app);
        return false;
    }
    crate::dictee::log_line(app, &format!("son_baisse avant={avant} pendant={pendant}"));
    true
}

/// Exécute un petit script AppleScript (réglage du volume, sans autorisation
/// macOS particulière). Renvoie sa sortie, ou None en cas d'échec.
fn osascript(script: &str) -> Option<String> {
    let out = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
