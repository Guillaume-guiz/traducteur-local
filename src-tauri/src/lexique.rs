//! Mots à corriger, réglables depuis le panneau de la dictée :
//! quand la transcription écrit un mot de travers (« Monsieur Guise » pour
//! « Monsieur Guiz »), on indique ce qu'elle écrit et ce qu'on veut. Même
//! fichier que le lexique édité à la main (`lexique.txt` dans le dossier de
//! données de l'app) : les commentaires et l'ordre des lignes sont gardés.

use serde::Serialize;
use tauri::AppHandle;

use crate::dictee::{lexicon_path, lexicon_rule};

/// En-tête d'un lexique créé depuis le panneau.
const ENTETE: &str = "# Lexique de la dictée : une correction par ligne, « ce qui sort => ce qu'on veut ».\n\
# Appliqué à chaque dictée, avant la traduction éventuelle.\n\
# Majuscules et minuscules indifférentes, mots entiers seulement.\n\
# Pris en compte à la dictée suivante, sans relancer l'app.\n";

#[derive(Serialize)]
pub struct Entree {
    de: String,
    vers: String,
}

fn texte(app: &AppHandle) -> String {
    lexicon_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

/// Corrections du lexique, dans l'ordre du fichier.
pub fn lire(app: &AppHandle) -> Vec<Entree> {
    texte(app)
        .lines()
        .filter_map(lexicon_rule)
        .map(|(de, vers)| Entree {
            de: de.to_string(),
            vers: vers.to_string(),
        })
        .collect()
}

fn ecrire(app: &AppHandle, lignes: &[String]) -> Result<(), String> {
    let path = lexicon_path(app).ok_or("Lexique introuvable.")?;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&path, lignes.join("\n") + "\n")
        .map_err(|e| format!("Lexique non enregistré ({e})."))
}

fn meme_mot(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Ajoute une correction, ou remplace celle qui porte déjà sur le même mot.
#[tauri::command]
pub fn ajouter_lexique(app: AppHandle, de: String, vers: String) -> Result<Vec<Entree>, String> {
    let (de, vers) = (de.trim(), vers.trim());
    if de.is_empty() {
        return Err("Indique ce que la dictée écrit.".into());
    }
    if vers.is_empty() {
        return Err("Indique ce que tu veux à la place.".into());
    }
    if de.contains("=>") || vers.contains("=>") {
        return Err("Sans « => » dans les mots.".into());
    }
    if de.starts_with('#') {
        return Err("Un mot ne peut pas commencer par #.".into());
    }
    if de == vers {
        return Err("Les deux sont identiques.".into());
    }
    ecrire(&app, &avec_correction(&texte(&app), de, vers))?;
    Ok(lire(&app))
}

/// Lignes du lexique après ajout : une seule correction par mot, la nouvelle
/// prend la place de l'ancienne ; commentaires et ordre gardés.
fn avec_correction(existant: &str, de: &str, vers: &str) -> Vec<String> {
    let existant = if existant.trim().is_empty() { ENTETE } else { existant };
    let nouvelle = format!("{de} => {vers}");
    let mut lignes: Vec<String> = Vec::new();
    let mut placee = false;
    for l in existant.lines() {
        match lexicon_rule(l) {
            Some((d, _)) if meme_mot(d, de) => {
                if !placee {
                    lignes.push(nouvelle.clone());
                    placee = true;
                }
            }
            _ => lignes.push(l.to_string()),
        }
    }
    if !placee {
        lignes.push(nouvelle);
    }
    lignes
}

fn sans_correction(existant: &str, de: &str) -> Vec<String> {
    existant
        .lines()
        .filter(|l| !matches!(lexicon_rule(l), Some((d, _)) if meme_mot(d, de)))
        .map(String::from)
        .collect()
}

/// Retire la correction qui porte sur ce mot.
#[tauri::command]
pub fn retirer_lexique(app: AppHandle, de: String) -> Result<Vec<Entree>, String> {
    ecrire(&app, &sans_correction(&texte(&app), &de))?;
    Ok(lire(&app))
}

#[cfg(test)]
mod tests {
    use super::{avec_correction, sans_correction};

    const LEXIQUE: &str = "# commentaire\nJ S => JS\nIman => Hymane\n";

    #[test]
    fn ajoute_en_fin_et_garde_les_commentaires() {
        assert_eq!(
            avec_correction(LEXIQUE, "Monsieur Guise", "Monsieur Guiz"),
            vec!["# commentaire", "J S => JS", "Iman => Hymane", "Monsieur Guise => Monsieur Guiz"]
        );
    }

    #[test]
    fn remplace_la_correction_du_meme_mot_sur_place() {
        assert_eq!(
            avec_correction(LEXIQUE, "iman", "Imane"),
            vec!["# commentaire", "J S => JS", "iman => Imane"]
        );
    }

    #[test]
    fn retire_sans_toucher_au_reste() {
        assert_eq!(sans_correction(LEXIQUE, "j s"), vec!["# commentaire", "Iman => Hymane"]);
    }

    #[test]
    fn cree_l_en_tete_si_le_lexique_est_vide() {
        let l = avec_correction("", "Guise", "Guiz");
        assert!(l[0].starts_with("# Lexique"));
        assert_eq!(l.last().unwrap(), "Guise => Guiz");
    }
}
