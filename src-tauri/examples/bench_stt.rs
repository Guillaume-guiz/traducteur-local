//! Mesure la vitesse de transcription locale sur des fichiers WAV (16 kHz mono).
//! Usage : cargo run --release --example bench_stt -- <moteur> <dossier_modele> <fichier.wav>...
//! Moteurs : `parakeet` (langue devinée) ; `canary` (langue forcée en français) ;
//! `canary-en` (français traduit en anglais par le moteur lui-même). Comparaison
//! du 30/09/2026 : Parakeet bascule parfois en anglais sur du français truffé de
//! mots anglais, Canary peut être forcé en français.
use std::path::PathBuf;
use std::time::Instant;
use transcribe_rs::onnx::canary::{CanaryModel, CanaryParams};
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};
use transcribe_rs::onnx::Quantization;

enum Moteur {
    Parakeet(ParakeetModel),
    Canary(CanaryModel, &'static str),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let model_dir = PathBuf::from(&args[2]);
    let t = Instant::now();
    let mut moteur = match args[1].as_str() {
        "parakeet" => Moteur::Parakeet(ParakeetModel::load(&model_dir, &Quantization::Int8)?),
        "canary" => Moteur::Canary(CanaryModel::load(&model_dir, &Quantization::Int8)?, "fr"),
        "canary-en" => Moteur::Canary(CanaryModel::load(&model_dir, &Quantization::Int8)?, "en"),
        autre => return Err(format!("moteur inconnu : {autre}").into()),
    };
    println!("chargement_ms={}", t.elapsed().as_millis());
    for wav in &args[3..] {
        let samples = transcribe_rs::audio::read_wav_samples(&PathBuf::from(wav))?;
        let secs = samples.len() as f32 / 16000.0;
        for run in 0..2 {
            let t = Instant::now();
            let texte = match &mut moteur {
                Moteur::Parakeet(m) => m.transcribe_with(&samples, &ParakeetParams::default())?.text,
                Moteur::Canary(m, cible) => {
                    let params = CanaryParams {
                        language: Some("fr".into()),
                        target_language: Some((*cible).into()),
                        ..CanaryParams::default()
                    };
                    m.transcribe_with(&samples, &params)?.text
                }
            };
            println!("{wav} run={run} parole={secs:.1}s transcription_ms={}", t.elapsed().as_millis());
            if run == 1 {
                println!("  => {}", texte.trim());
            }
        }
    }
    Ok(())
}
