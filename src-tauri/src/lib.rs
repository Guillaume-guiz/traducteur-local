use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

mod controle;
mod decoupe;
pub mod dictee;
mod hud;
mod lexique;
mod pastille;
mod popover;
mod raccourcis;
mod son;

pub(crate) const OLLAMA_URL: &str = "http://localhost:11434";
/// Raccourci global par défaut, utilisé tant que l'utilisateur n'en a pas choisi un.
pub(crate) const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+T";

/// État partagé : raccourcis globaux courants (source de vérité côté Rust, pour
/// pouvoir désenregistrer l'ancien avant d'enregistrer le nouveau).
pub(crate) struct AppState {
    /// Traduire la sélection.
    pub(crate) shortcut: Mutex<String>,
    /// Langue de la dictée suivante / précédente parmi les trois boutons rapides,
    /// sans ouvrir le panneau (réglables depuis le 29/09/2026).
    pub(crate) suivante: Mutex<String>,
    pub(crate) precedente: Mutex<String>,
}

/// Chemin du fichier de réglages (~/Library/Application Support/<id>/settings.json).
fn settings_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("settings.json"))
}

/// Lit l'objet de réglages persistant (vide s'il n'existe pas encore).
pub(crate) fn read_settings(app: &AppHandle) -> serde_json::Map<String, Value> {
    if let Some(path) = settings_path(app) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&content) {
                return map;
            }
        }
    }
    serde_json::Map::new()
}

/// Réécrit l'objet de réglages complet (best effort).
pub(crate) fn write_settings(app: &AppHandle, map: &serde_json::Map<String, Value>) {
    if let Some(path) = settings_path(app) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(body) = serde_json::to_string_pretty(&Value::Object(map.clone())) {
            let _ = std::fs::write(path, body);
        }
    }
}

/// Met à jour une seule clé de réglage sans écraser les autres.
pub(crate) fn save_setting(app: &AppHandle, key: &str, value: &str) {
    let mut map = read_settings(app);
    map.insert(key.to_string(), Value::String(value.to_string()));
    write_settings(app, &map);
}

/// Durée pendant laquelle le micro reste ouvert après une dictée (réglage
/// `mic_warm_secs`, en secondes ; 0 = le fermer tout de suite, -1 = toujours).
/// Défaut 0 : avec un casque Bluetooth, un micro gardé ouvert laisse le casque
/// en mode appel (son dégradé) ; essayé à 30 s le 27/09/2026, retiré aussitôt.
pub(crate) fn mic_warm_secs(app: &AppHandle) -> i64 {
    read_settings(app)
        .get("mic_warm_secs")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

/// Lit le raccourci enregistré, ou renvoie le défaut.
fn load_saved_shortcut(app: &AppHandle) -> String {
    let s = read_settings(app)
        .get("shortcut")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if s.is_empty() {
        DEFAULT_SHORTCUT.to_string()
    } else {
        s
    }
}

/// Modèle Ollama mémorisé, ou le défaut du projet.
pub(crate) fn saved_model_or_default(app: &AppHandle) -> String {
    read_settings(app)
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("mistral:latest")
        .to_string()
}

/// Modèle de la dictée (réglage `dictation_model`). Défaut : Aya Expanse 8B,
/// le plus fidèle en espagnol et en allemand, gros mots et heures compris ; à peine
/// plus lent que Gemma 3 4B (comparatif dans CLAUDE.md, section Décisions).
pub(crate) fn dictation_model(app: &AppHandle) -> String {
    read_settings(app)
        .get("dictation_model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("aya-expanse:8b")
        .to_string()
}

/// Langue cible de la dictée traduite (réglage `dictation_target`, défaut anglais).
pub(crate) fn dictation_target(app: &AppHandle) -> String {
    read_settings(app)
        .get("dictation_target")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("English")
        .to_string()
}

/// Raccourci de dictée enregistré (réglage `dictation_shortcut`), ou le défaut.
pub(crate) fn load_dictation_shortcut(app: &AppHandle) -> String {
    read_settings(app)
        .get("dictation_shortcut")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(dictee::DEFAULT_DICTATION_SHORTCUT)
        .to_string()
}

/// Persiste le raccourci choisi (best effort).
fn save_shortcut(app: &AppHandle, shortcut: &str) {
    save_setting(app, "shortcut", shortcut);
}

/// Caractéristiques machine, pour recommander des modèles adaptés au matériel.
#[derive(serde::Serialize)]
struct SystemInfo {
    ram_gb: u64,
    free_disk_gb: u64,
    chip: String,
    arch: String,
}

/// Lecture d'une clé sysctl (macOS).
fn sysctl_str(key: &str) -> Option<String> {
    std::process::Command::new("sysctl")
        .args(["-n", key])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Espace disque libre (Go) sur le volume du home (là où vivent les modèles Ollama).
fn free_disk_gb() -> Option<u64> {
    let home = std::env::var("HOME").ok()?;
    let out = std::process::Command::new("df")
        .args(["-k", &home])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let line = text.lines().nth(1)?; // ligne de données (après l'en-tête)
    let avail_k: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(avail_k / 1_048_576) // blocs de 1 Ko → Go
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: core_foundation::string::CFStringRef;
}

/// L'app a-t-elle l'autorisation macOS « Accessibilité » ? (requise pour simuler
/// ⌘C / ⌘V vers une autre application).
pub(crate) fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Déclenche la pop-up macOS d'autorisation « Accessibilité » et inscrit l'app
/// dans la liste des Réglages Système. Renvoie true si déjà accordée.
pub(crate) fn request_accessibility_prompt() -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let value = CFBoolean::true_value();
        let opts = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
        AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef())
    }
}

/// Poste un raccourci ⌘+<touche> au système (keycodes ANSI : C = 8, V = 9).
pub(crate) fn post_cmd_key(keycode: u16) {
    if let Ok(src) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
        if let Ok(ev) = CGEvent::new_keyboard_event(src, keycode, true) {
            ev.set_flags(CGEventFlags::CGEventFlagCommand);
            ev.post(CGEventTapLocation::HID);
        }
    }
    if let Ok(src) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
        if let Ok(ev) = CGEvent::new_keyboard_event(src, keycode, false) {
            ev.set_flags(CGEventFlags::CGEventFlagCommand);
            ev.post(CGEventTapLocation::HID);
        }
    }
}

/// Appel générique au endpoint /api/chat d'Ollama (modèle local).
pub(crate) async fn ollama_chat(
    model: &str,
    system: &str,
    user: &str,
    temperature: f32,
) -> Result<String, String> {
    // Timeout client : un modèle qui part en génération sans fin ne doit jamais
    // bloquer l'UI indéfiniment ; on échoue proprement après 120 s.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({
        "model": model,
        "stream": false,
        // Garde le modèle en mémoire 30 min : évite de le recharger à chaque dictée.
        "keep_alive": "30m",
        "options": {
            "temperature": temperature,
            // Plafond de génération : borne le temps de réponse même si un modèle
            // mal packagé ne s'arrête pas tout seul.
            "num_predict": 1024,
            // Tokens d'arrêt usuels (modèles à template ChatML notamment).
            "stop": ["<|im_end|>", "<|endoftext|>"]
        },
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    let resp = client
        .post(format!("{OLLAMA_URL}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Le modèle met trop de temps à répondre (timeout 120 s). Il génère peut-être sans fin : essaie un autre modèle.".to_string()
            } else {
                format!("Connexion à Ollama impossible ({e}). Ollama est-il lancé ?")
            }
        })?;
    if !resp.status().is_success() {
        return Err(format!("Ollama a répondu : {}", resp.status()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(v["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// Traduit `text` vers `target_lang` avec un registre (`tone`) donné.
#[tauri::command]
async fn translate(
    text: String,
    source_lang: String,
    target_lang: String,
    tone: String,
    model: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let tone_line = match tone.as_str() {
        "formal" => "Use a formal, polished register (in French, use vouvoiement).",
        "informal" => "Use an informal, friendly register (in French, use tutoiement).",
        _ => "Match the register and tone of the source text.",
    };
    let source_clause = if source_lang.is_empty() || source_lang == "auto" {
        "Automatically detect the source language and translate the user's message".to_string()
    } else {
        format!("Translate the user's message from {source_lang}")
    };
    let system = format!(
        "You are a professional translation engine. {source_clause} into {target_lang}. {tone_line} \
         Output ONLY the translation, with no preamble, no explanation, no quotes and no notes. \
         Preserve line breaks, lists and basic formatting."
    );
    ollama_chat(&model, &system, &text, 0.3).await
}

/// Reformule `text` (DeepL Write) dans la même langue, selon le `mode`.
#[tauri::command]
async fn rephrase(text: String, mode: String, model: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let instruction = match mode.as_str() {
        "formal" => "Rewrite it in a more formal, professional register.",
        "casual" => "Rewrite it in a warmer, more casual register.",
        "shorten" => "Rewrite it to be clearly more concise, keeping all key information.",
        "expand" => "Rewrite it to be a little more detailed and developed.",
        _ => "Rewrite it to improve clarity, flow and overall style.",
    };
    let system = format!(
        "You are a professional writing assistant. {instruction} Keep the SAME language as the input. \
         Output ONLY the rewritten text, with no preamble, no quotes and no explanation."
    );
    ollama_chat(&model, &system, &text, 0.5).await
}

/// Propose plusieurs variantes de traduction pour une sélection.
#[tauri::command]
async fn alternatives(
    text: String,
    target_lang: String,
    model: String,
) -> Result<Vec<String>, String> {
    if text.trim().is_empty() {
        return Ok(vec![]);
    }
    let system = format!(
        "You are a professional translation engine. Provide exactly 4 alternative translations into \
         {target_lang} of the user's text, each with different word choices or phrasing but the same \
         meaning. Return ONLY a JSON array of 4 strings, nothing else."
    );
    let raw = ollama_chat(&model, &system, &text, 0.8).await?;
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    // Cas idéal : un tableau JSON propre.
    if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(cleaned) {
        let out: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();
        if !out.is_empty() {
            return Ok(out);
        }
    }
    // Fallback : une variante par ligne, en retirant puces/numéros/guillemets.
    let out: Vec<String> = cleaned
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches(|c: char| {
                    c.is_numeric() || c == '-' || c == '.' || c == ')' || c == '*' || c == ' '
                })
                .trim_matches('"')
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect();
    Ok(out)
}

/// Liste les modèles disponibles dans Ollama (pour le sélecteur de modèle).
#[tauri::command]
async fn list_models() -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{OLLAMA_URL}/api/tags"))
        .send()
        .await
        .map_err(|e| format!("Connexion à Ollama impossible ({e})."))?;
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let models = v["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(models)
}

/// Réinjecte la traduction dans le champ où l'utilisateur était : écrit la
/// traduction dans le presse-papier, masque la fenêtre (le focus revient à l'app
/// précédente), puis simule ⌘V. Nécessite l'autorisation « Accessibilité ».
#[tauri::command]
fn insert_translation(app: AppHandle, text: String) -> Result<bool, String> {
    if text.trim().is_empty() {
        return Ok(false);
    }
    // Toujours copier la traduction dans le presse-papier AVANT tout : c'est à la
    // fois la demande explicite et le fallback si l'insertion auto échoue (on peut
    // alors coller à la main avec ⌘V).
    app.clipboard()
        .write_text(text)
        .map_err(|e| e.to_string())?;
    // L'insertion automatique (⌘V simulé) exige l'autorisation Accessibilité.
    // Sans elle, on s'arrête là : le texte est déjà dans le presse-papier.
    if !accessibility_trusted() {
        return Err("ACCESSIBILITY_REQUIRED".into());
    }
    // Cacher l'APPLICATION entière (pas seulement la fenêtre) : sur macOS c'est ce
    // qui réactive l'app précédente et lui rend le focus clavier. Sans ça, le ⌘V
    // simulé n'atterrit dans aucun champ.
    let _ = app.hide();
    // Le collage se fait sur un thread séparé après un court délai, pour laisser
    // macOS réactiver l'app précédente avant le ⌘V.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(350));
        post_cmd_key(9); // ⌘V
    });
    Ok(true)
}

#[tauri::command]
fn check_accessibility() -> bool {
    accessibility_trusted()
}

#[tauri::command]
fn request_accessibility() -> bool {
    request_accessibility_prompt()
}

#[tauri::command]
fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// Renvoie le raccourci global courant (format accélérateur Tauri, ex "CmdOrCtrl+Shift+T").
#[tauri::command]
fn get_shortcut(state: State<AppState>) -> String {
    state
        .shortcut
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| DEFAULT_SHORTCUT.to_string())
}

/// Change le raccourci global : désenregistre l'ancien, enregistre le nouveau,
/// persiste. En cas de refus système, restaure l'ancien et renvoie une erreur.
#[tauri::command]
fn set_shortcut(app: AppHandle, state: State<AppState>, shortcut: String) -> Result<String, String> {
    let next = shortcut.trim().to_string();
    if next.is_empty() {
        return Err("Raccourci vide.".into());
    }
    let gs = app.global_shortcut();
    let mut cur = state
        .shortcut
        .lock()
        .map_err(|_| "État interne indisponible.".to_string())?;
    // Désenregistre l'ancien (ignore l'erreur s'il n'était déjà plus actif).
    let _ = gs.unregister(cur.as_str());
    match gs.register(next.as_str()) {
        Ok(_) => {
            *cur = next.clone();
            save_shortcut(&app, &next);
            Ok(next)
        }
        Err(e) => {
            // Restaure l'ancien pour ne pas laisser l'app sans raccourci.
            let _ = gs.register(cur.as_str());
            Err(format!(
                "Raccourci refusé par le système ({e}). Essaie une autre combinaison."
            ))
        }
    }
}

/// Désactive temporairement le raccourci courant (pendant la capture d'un nouveau,
/// pour que l'ancien ne se déclenche pas sous les doigts de l'utilisateur).
#[tauri::command]
fn pause_shortcut(app: AppHandle, state: State<AppState>) {
    if let Ok(cur) = state.shortcut.lock() {
        let _ = app.global_shortcut().unregister(cur.as_str());
    }
}

/// Réactive le raccourci courant (capture annulée ou terminée sans enregistrement).
#[tauri::command]
fn resume_shortcut(app: AppHandle, state: State<AppState>) {
    if let Ok(cur) = state.shortcut.lock() {
        // register renvoie une erreur s'il est déjà actif : sans danger, on l'ignore.
        let _ = app.global_shortcut().register(cur.as_str());
    }
}

/// Renvoie les caractéristiques machine (RAM, disque libre, puce, architecture).
#[tauri::command]
fn system_info() -> SystemInfo {
    let ram_gb = sysctl_str("hw.memsize")
        .and_then(|s| s.parse::<u64>().ok())
        .map(|bytes| bytes / 1_073_741_824)
        .unwrap_or(0);
    SystemInfo {
        ram_gb,
        free_disk_gb: free_disk_gb().unwrap_or(0),
        chip: sysctl_str("machdep.cpu.brand_string").unwrap_or_else(|| "Mac".to_string()),
        arch: std::env::consts::ARCH.to_string(),
    }
}

/// Renvoie le dernier modèle utilisé, mémorisé sur disque (vide si aucun).
#[tauri::command]
fn get_saved_model(app: AppHandle) -> String {
    read_settings(&app)
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Mémorise le modèle choisi.
#[tauri::command]
fn set_saved_model(app: AppHandle, model: String) {
    save_setting(&app, "model", &model);
}

/// Télécharge un modèle via Ollama (/api/pull) et streame la progression vers
/// le frontend via l'événement "pull-progress" (puis "pull-done").
#[tauri::command]
async fn pull_model(app: AppHandle, name: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    let mut resp = client
        .post(format!("{OLLAMA_URL}/api/pull"))
        .json(&serde_json::json!({ "name": name, "stream": true }))
        .send()
        .await
        .map_err(|e| format!("Connexion à Ollama impossible ({e})."))?;
    if !resp.status().is_success() {
        return Err(format!("Ollama a répondu : {}", resp.status()));
    }
    // La réponse est du NDJSON : on accumule et on découpe ligne par ligne.
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line = &line[..line.len().saturating_sub(1)];
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_slice::<Value>(line) {
                if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
                    return Err(err.to_string());
                }
                let total = v.get("total").and_then(|x| x.as_u64()).unwrap_or(0);
                let completed = v.get("completed").and_then(|x| x.as_u64()).unwrap_or(0);
                let percent = if total > 0 {
                    (completed as f64 / total as f64 * 100.0).round() as u32
                } else {
                    0
                };
                let _ = app.emit(
                    "pull-progress",
                    serde_json::json!({
                        "model": name,
                        "status": v.get("status").and_then(|x| x.as_str()).unwrap_or(""),
                        "completed": completed,
                        "total": total,
                        "percent": percent,
                    }),
                );
            }
        }
    }
    let _ = app.emit("pull-done", serde_json::json!({ "model": name }));
    Ok(())
}

/// Supprime un modèle d'Ollama (/api/delete) pour libérer de l'espace disque.
#[tauri::command]
async fn delete_model(name: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .delete(format!("{OLLAMA_URL}/api/delete"))
        .json(&serde_json::json!({ "name": name }))
        .send()
        .await
        .map_err(|e| format!("Connexion à Ollama impossible ({e})."))?;
    if !resp.status().is_success() {
        return Err(format!("Suppression impossible ({}).", resp.status()));
    }
    Ok(())
}

/// Ramène la fenêtre principale au premier plan (réaffiche si cachée, dé-minimise,
/// donne le focus). Utilisé par l'icône menu bar et le raccourci global.
pub(crate) fn show_main_window(app: &AppHandle) {
    // Ré-affiche l'app si elle a été cachée par l'Insérer (app.hide()), puis remonte
    // la fenêtre et lui donne le focus.
    let _ = app.show();
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Déclenché par le raccourci global : copie la sélection courante (si
/// l'accessibilité est accordée), lit le presse-papier, montre la fenêtre et
/// pousse le texte au frontend pour traduction immédiate.
fn handle_clipboard_shortcut(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if accessibility_trusted() {
            post_cmd_key(8); // ⌘C : copie la sélection de l'app active (façon DeepL)
            std::thread::sleep(Duration::from_millis(120));
        }
        let text = app.clipboard().read_text().unwrap_or_default();
        show_main_window(&app);
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.emit("clipboard-translate", text);
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // Raccourci de dictée : on écoute tant qu'il est maintenu.
                    let is_dictation = app
                        .try_state::<dictee::DictationState>()
                        .and_then(|st| st.shortcut.lock().ok().map(|s| s.clone()))
                        .and_then(|s| s.parse::<Shortcut>().ok())
                        .map(|d| d.id() == shortcut.id())
                        .unwrap_or(false);
                    if is_dictation {
                        match event.state() {
                            ShortcutState::Pressed => dictee::on_press(app, 150),
                            ShortcutState::Released => dictee::on_release(app),
                        }
                        return;
                    }
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let matches = |slot: fn(&AppState) -> &Mutex<String>| {
                        app.try_state::<AppState>()
                            .and_then(|st| slot(&st).lock().ok().map(|s| s.clone()))
                            .and_then(|s| s.parse::<Shortcut>().ok())
                            .map(|p| p.id() == shortcut.id())
                            .unwrap_or(false)
                    };
                    if matches(|st| &st.suivante) {
                        popover::cycle_target(app, 1, true);
                    } else if matches(|st| &st.precedente) {
                        popover::cycle_target(app, -1, true);
                    } else {
                        handle_clipboard_shortcut(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            translate,
            rephrase,
            alternatives,
            list_models,
            insert_translation,
            check_accessibility,
            request_accessibility,
            open_accessibility_settings,
            get_shortcut,
            set_shortcut,
            pause_shortcut,
            resume_shortcut,
            system_info,
            get_saved_model,
            set_saved_model,
            pull_model,
            delete_model,
            popover::get_dictation_config,
            popover::set_dictation_target,
            popover::set_quick_targets,
            popover::set_mic_warm_secs,
            popover::set_son_dictee,
            raccourcis::set_raccourci,
            lexique::ajouter_lexique,
            lexique::retirer_lexique,
            raccourcis::suspendre_raccourcis,
            popover::close_popover,
            popover::fit_popover,
            popover::open_translator,
            popover::quit_app,
            pastille::pastille_etat,
            pastille::pastille_ajuster,
            pastille::pastille_glisser,
            pastille::pastille_langue_suivante,
            pastille::pastille_agrandir,
            pastille::pastille_panneau
        ])
        .setup(|app| {
            // Raccourci global : sélectionne du texte, puis le raccourci (défaut ⌘⇧T,
            // personnalisable depuis les réglages et persisté sur disque).
            let handle = app.handle().clone();
            // Son baissé pendant la dictée ; rend d'abord un volume resté bas si
            // l'app s'est arrêtée en pleine dictée.
            son::demarrer(&handle);
            let initial = load_saved_shortcut(&handle);
            if let Err(e) = app.global_shortcut().register(initial.as_str()) {
                eprintln!("Impossible d'enregistrer le raccourci global : {e}");
            }
            // Changer la langue de la dictée au clavier, sans ouvrir le panneau.
            let suivante = raccourcis::suivante_enregistree(&handle);
            let precedente = raccourcis::precedente_enregistree(&handle);
            for (nom, r) in [("langue suivante", &suivante), ("langue précédente", &precedente)] {
                if let Err(e) = app.global_shortcut().register(r.as_str()) {
                    eprintln!("Impossible d'enregistrer le raccourci {nom} : {e}");
                }
            }
            app.manage(AppState {
                shortcut: Mutex::new(initial),
                suivante: Mutex::new(suivante),
                precedente: Mutex::new(precedente),
            });

            // Dictée traduite : touche à maintenir (défaut Contrôle seul), modèle de
            // transcription préchargé en arrière-plan.
            let dictation = load_dictation_shortcut(&handle);
            let control_mode = dictee::is_control_mode(&dictation);
            if !control_mode {
                if let Err(e) = app.global_shortcut().register(dictation.as_str()) {
                    eprintln!("Impossible d'enregistrer le raccourci de dictée : {e}");
                }
            }
            app.manage(dictee::DictationState::new(dictation));
            // Contrôle seul, maintenu : écoute du clavier au niveau système.
            controle::activer(&handle, control_mode);
            dictee::preload(&handle);
            #[cfg(debug_assertions)]
            dictee::self_test(&handle);

            hud::create(&handle)?;
            // Pastille flottante : état de la dictée, toujours visible.
            pastille::create(&handle)?;

            // Icône dans la barre de menus macOS (tray).
            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
                let show_i =
                    MenuItem::with_id(app, "show", "Afficher le traducteur", true, None::<&str>)?;
                let settings_i = MenuItem::with_id(
                    app,
                    "settings",
                    "Modifier le raccourci…",
                    true,
                    None::<&str>,
                )?;
                let quit_i = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;

                let menu = Menu::with_items(app, &[&show_i, &settings_i, &quit_i])?;
                // Panneau de dictée (trois langues + réglages), ouvert au clic sur l'icône.
                popover::create(app.handle())?;
                let tray = TrayIconBuilder::with_id("main-tray")
                    .icon(app.default_window_icon().unwrap().clone())
                    .menu(&menu)
                    // Clic gauche = panneau de dictée sous l'icône ; le menu système
                    // (afficher le traducteur, raccourci, quitter) reste au clic droit.
                    .show_menu_on_left_click(false)
                    .tooltip("Traducteur local")
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            rect,
                            ..
                        } = event
                        {
                            popover::toggle(tray.app_handle(), rect);
                        }
                    })
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "show" => show_main_window(app),
                        "settings" => {
                            show_main_window(app);
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.emit("open-settings", ());
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .build(app)?;
                // Garder l'icône en vie : sans ça, elle est détruite (et disparaît
                // de la barre de menus) dès la fin de ce bloc `setup`.
                app.manage(tray);
            }
            Ok(())
        })
        // Fermer la fenêtre la cache au lieu de quitter (l'app reste dans la barre de menus).
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let _ = window.hide();
                api.prevent_close();
            }
            // Le panneau de dictée se ferme dès qu'on clique ailleurs.
            tauri::WindowEvent::Focused(false) if window.label() == popover::LABEL => {
                popover::on_blur(window);
            }
            // La pastille glissée à la souris : sa nouvelle place est retenue.
            tauri::WindowEvent::Moved(pos) if window.label() == pastille::LABEL => {
                pastille::deplacee(window, pos);
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        // La fenêtre du traducteur reste cachée au démarrage (l'app s'ouvre avec
        // le Mac) : elle s'ouvre par le panneau de l'icône, le menu du clic droit,
        // le raccourci de traduction, ou un clic sur l'icône du Dock.
        .run(|app, event| match event {
            tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => show_main_window(app),
            // Quitter en pleine dictée ne doit pas laisser le son baissé.
            tauri::RunEvent::Exit => son::remonter_maintenant(app),
            _ => {}
        });
}
