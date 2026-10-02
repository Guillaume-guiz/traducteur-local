//! Découpage de la dictée aux pauses (30/09/2026, demande de Guillaume) :
//! pendant qu'on parle, chaque morceau d'au moins 8 s est transcrit dès
//! qu'une pause apparaît. Au relâchement, il ne reste que la fin à
//! transcrire : l'attente raccourcit sur les longues dictées, et un
//! basculement de langue du moteur (vu le 30/09 : une dictée en français
//! sortie en anglais) reste limité à un morceau.

use std::cmp::Ordering as Ordre;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// Un morceau dure au moins ça : assez de contexte pour que le moteur
/// reconnaisse la langue.
const MORCEAU_MIN_S: f32 = 8.0;
/// Au-delà, on coupe sur une pause plus courte, ou au moment le plus calme.
const MORCEAU_MAX_S: f32 = 25.0;
/// Le son est analysé par tranches de 20 ms.
const TRAME_MS: usize = 20;
/// Silence qui permet de couper (le second au-delà de la durée maximale). Les
/// pauses entre deux phrases durent de 300 à 700 ms environ.
const PAUSE_MS: usize = 300;
const PAUSE_COURTE_MS: usize = 200;

#[derive(Default)]
pub struct Etat {
    /// Début, dans le tampon du micro, de ce qui n'est pas encore transcrit.
    pub coupe: usize,
    /// Textes des morceaux déjà transcrits, dans l'ordre.
    pub textes: Vec<String>,
    /// Temps passé à transcrire pendant que l'on parlait (journal).
    pub stt_ms: u128,
}

pub struct Decoupe {
    stop: AtomicBool,
    pub etat: Mutex<Etat>,
    fil: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Decoupe {
    /// Lance le découpage d'un enregistrement qui commence à `debut` dans le
    /// tampon du micro (échantillons entrelacés).
    pub fn lancer(
        app: &AppHandle,
        buf: Arc<Mutex<Vec<f32>>>,
        rate: u32,
        channels: u16,
        debut: usize,
    ) -> Arc<Self> {
        let d = Arc::new(Decoupe {
            stop: AtomicBool::new(false),
            etat: Mutex::new(Etat {
                coupe: debut,
                ..Default::default()
            }),
            fil: Mutex::new(None),
        });
        let (d2, app) = (d.clone(), app.clone());
        let fil = std::thread::spawn(move || boucle(&app, &d2, &buf, rate, channels, debut));
        *d.fil.lock().unwrap() = Some(fil);
        d
    }

    /// Arrête le découpage ; `attendre` laisse finir le morceau en cours de
    /// transcription (au relâchement), pas à l'abandon.
    pub fn arreter(&self, attendre: bool) {
        self.stop.store(true, Ordering::SeqCst);
        if attendre {
            let fil = self.fil.lock().unwrap().take();
            if let Some(fil) = fil {
                let _ = fil.join();
            }
        }
    }
}

fn boucle(app: &AppHandle, d: &Decoupe, buf: &Mutex<Vec<f32>>, rate: u32, channels: u16, debut: usize) {
    let ch = channels.max(1) as usize;
    // Tranche de 20 ms, en nombre d'échantillons entrelacés (multiple du nombre de canaux).
    let trame = (rate as usize * TRAME_MS / 1000).max(1) * ch;
    let par_s = 1000 / TRAME_MS;
    let (mut analyse, mut coupe) = (debut, debut);
    let mut niveaux: Vec<f32> = Vec::new();
    while !d.stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
        {
            let b = buf.lock().unwrap();
            while analyse + trame <= b.len() {
                niveaux.push(crate::dictee::rms(&b[analyse..analyse + trame]));
                analyse += trame;
            }
        }
        let secs = niveaux.len() as f32 / par_s as f32;
        if secs < MORCEAU_MIN_S {
            continue;
        }
        let Some(k) = point_de_coupe(&niveaux, par_s, secs >= MORCEAU_MAX_S) else {
            continue;
        };
        let fin = coupe + k * trame;
        let Some(samples) = buf.lock().unwrap().get(coupe..fin).map(<[f32]>::to_vec) else {
            break;
        };
        if d.stop.load(Ordering::SeqCst) {
            break;
        }
        let t = Instant::now();
        match crate::dictee::transcrire_morceau(app, samples, rate, channels) {
            Ok(texte) => {
                let mut e = d.etat.lock().unwrap();
                if !texte.is_empty() {
                    e.textes.push(texte);
                }
                e.stt_ms += t.elapsed().as_millis();
                e.coupe = fin;
            }
            Err(err) => {
                // Le reste sera transcrit d'un bloc au relâchement.
                crate::dictee::log_line(app, &format!("decoupe_echec {err}"));
                break;
            }
        }
        niveaux.drain(..k);
        coupe = fin;
    }
}

/// Où couper, en nombre de tranches depuis le début du morceau : au milieu de
/// la première pause assez longue après la durée minimale. `force` (morceau
/// trop long) accepte une pause plus courte, sinon le moment le plus calme.
/// Le silence se juge par rapport au bruit de fond et à la voix du morceau.
/// Seuil sous lequel une tranche compte comme du silence, d'après le morceau
/// lui-même : bruit de fond (10 % les plus calmes) et niveau de la voix (90 %) ;
/// trois fois le fond, au moins 8 % de la voix (fond numérique nul), au plus
/// 25 % (pauses rares, fond mal estimé).
pub(crate) fn seuil_silence(niveaux: &[f32]) -> f32 {
    let mut tries = niveaux.to_vec();
    tries.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordre::Equal));
    let fond = tries[tries.len() / 10];
    let voix = tries[tries.len() * 9 / 10];
    (fond * 3.0).max(voix * 0.08).max(0.003).min(voix * 0.25)
}

pub(crate) fn point_de_coupe(niveaux: &[f32], par_s: usize, force: bool) -> Option<usize> {
    if niveaux.is_empty() {
        return None;
    }
    let seuil = seuil_silence(niveaux);
    let min = (MORCEAU_MIN_S * par_s as f32) as usize;
    let pause = (if force { PAUSE_COURTE_MS } else { PAUSE_MS }) / TRAME_MS;
    let mut silence = 0;
    for (i, &n) in niveaux.iter().enumerate() {
        silence = if n < seuil { silence + 1 } else { 0 };
        if silence >= pause && i + 1 - pause / 2 >= min {
            return Some(i + 1 - pause / 2);
        }
    }
    if force {
        // Pas de pause : le moment le plus calme, hors de la dernière demi-seconde.
        let fin = niveaux.len().saturating_sub(par_s / 2);
        return niveaux
            .iter()
            .enumerate()
            .take(fin)
            .skip(min)
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(Ordre::Equal))
            .map(|(k, _)| k.max(1));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::point_de_coupe;

    const PAR_S: usize = 50;

    fn niveaux(parts: &[(f32, f32)]) -> Vec<f32> {
        parts
            .iter()
            .flat_map(|(secs, n)| std::iter::repeat(*n).take((secs * PAR_S as f32) as usize))
            .collect()
    }

    #[test]
    fn coupe_au_milieu_de_la_premiere_pause_apres_8_s() {
        let n = niveaux(&[(9.0, 0.05), (0.6, 0.001), (3.0, 0.05)]);
        let k = point_de_coupe(&n, PAR_S, false).unwrap();
        assert!((450..480).contains(&k), "coupe à {k}");
    }

    #[test]
    fn ignore_une_pause_avant_8_s_et_une_pause_trop_courte() {
        let n = niveaux(&[(4.0, 0.05), (0.6, 0.001), (5.0, 0.05), (0.18, 0.001), (1.0, 0.05)]);
        assert_eq!(point_de_coupe(&n, PAR_S, false), None);
    }

    #[test]
    fn morceau_trop_long_coupe_au_plus_calme() {
        let mut n = niveaux(&[(26.0, 0.05)]);
        n[1000] = 0.02;
        assert_eq!(point_de_coupe(&n, PAR_S, true), Some(1000));
    }

    #[test]
    fn le_silence_se_juge_sur_le_bruit_de_fond() {
        // Fond bruyant (0,01) : une pause à 0,01 entre des paroles à 0,08 compte.
        let n = niveaux(&[(8.5, 0.08), (0.6, 0.01), (1.0, 0.08), (1.0, 0.01)]);
        assert!(point_de_coupe(&n, PAR_S, false).is_some());
    }
}
