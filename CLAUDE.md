# Traducteur local : contexte pour Claude Code

App macOS de traduction et de dictée **100 % locale** : le texte et la voix ne sortent jamais de la machine. Mode d'emploi côté utilisateur : [README.md](README.md). Ce fichier décrit le code, les décisions et les pièges, pour qui veut le comprendre ou le modifier.

Version 1.0.0, figée le 02/10/2026. Les dates citées plus bas sont celles où chaque choix a été fait pendant le développement.

## Stack

| Brique | Choix |
|---|---|
| Shell de l'app | Tauri 2 (Rust + webview native macOS) |
| Interface | TypeScript sans framework + Vite |
| Traduction | Ollama local (`http://localhost:11434`), modèle choisi dans l'app |
| Modèle par défaut | fenêtre : le dernier choisi, sinon `aya-expanse:8b`, sinon un `gemma`, sinon le premier installé ; dictée : `aya-expanse:8b` (réglage `dictation_model`), repli sur le modèle choisi dans la fenêtre puis `mistral:latest` |
| Transcription | Parakeet TDT 0.6B v3 int8 (ONNX) via la crate `transcribe-rs`, embarqué dans l'app ; modèle dans `~/Library/Application Support/fr.monsieurguiz.traducteur-local/models/` |
| Micro | `cpal` |
| Toolchain | Rust 1.88 minimum, Node 18 minimum |

## Lancer, construire, tester

```bash
npm install
npm run tauri dev                 # mode dev, prérequis : Ollama lancé + un modèle
scripts/installer.sh              # construit, signe, installe dans /Applications, lance
scripts/preparer.sh [modele]      # modèles seuls : Parakeet + un modèle Ollama
cd src-tauri && cargo test --lib  # tests unitaires (découpage, lexique, nettoyage)
```

Test de la dictée sans micro, en dev : `DICTEE_TEST_WAV=a.wav,b.wav npm run tauri dev` (WAV 16 kHz mono). Mesure de vitesse de transcription : `cargo run --release --example bench_stt -- parakeet <dossier_du_modele> <fichier.wav>`.

## Architecture

Quatre fenêtres, chacune une page HTML à la racine (déclarées dans `vite.config.ts`) :

| Page | Fenêtre | Côté Rust | Côté interface |
|---|---|---|---|
| `index.html` | le traducteur (cachée au démarrage) | `lib.rs` | `src/main.ts`, `src/styles.css` |
| `popover.html` | panneau de la dictée sous l'icône de la barre de menus | `popover.rs` | `src/popover.ts`, `src/popover.css` |
| `pastille.html` | pastille flottante, état de la dictée | `pastille.rs` | `src/pastille.ts`, `src/pastille.css` |
| `hud.html` | encart temporaire au changement de langue | `hud.rs` | `src/hud.ts`, `src/hud.css` |

Modules Rust (`src-tauri/src/`) :

- `lib.rs` : commandes Tauri de la fenêtre (`translate`, `rephrase`, `alternatives`, `list_models`, `pull_model`, `delete_model`, `system_info`, `insert_translation`, accessibilité, raccourci), icône de la barre de menus, appels Ollama par `reqwest` (timeout 120 s, `num_predict` plafonné, tokens d'arrêt), simulation de Cmd+C / Cmd+V par `core-graphics`, réglages dans `settings.json` (dossier de config de l'app).
- `dictee.rs` : la dictée. Appui = ouverture du micro et réveil du modèle Ollama ; relâchement = transcription locale, lexique, traduction (consigne dédiée à l'oral, préambule éventuel retiré), collage Cmd+V dans le champ actif, puis restauration du presse-papier. Liste des sorties dans `TARGETS`. Journal des durées (jamais le texte) : `dictee.log`.
- `decoupe.rs` : transcription par morceaux pendant qu'on parle. Le son est analysé par tranches de 20 ms ; passé 8 s, un morceau est coupé au milieu de la première pause de 300 ms (200 ms au-delà de 25 s, sinon au moment le plus calme) et transcrit aussitôt. Le silence se juge par rapport au morceau : trois fois le bruit de fond, entre 8 % et 25 % du niveau de la voix. Au relâchement, seule la fin reste à transcrire (0,44 s au lieu de 1,4 s sur 29 s de parole).
- `controle.rs` : dictée à la touche Ctrl seule, maintenue (réglage `dictation_shortcut` = `Control`, défaut). Event tap actif : Ctrl tenu 200 ms sans autre touche ouvre le micro, relâché termine, toute autre touche pendant l'appui annule (c'était un raccourci comme Ctrl+C). Un clic n'annule que dans les 200 premières ms (Ctrl+clic = clic droit) ; ensuite il place le curseur, Ctrl retiré du clic. Ne lit jamais les touches tapées, seulement leur type.
- `raccourcis.rs` (+ `src/raccourcis.ts`) : raccourcis réglables (dicter, langue suivante / précédente, traduire la sélection). Pendant la saisie d'une combinaison, tous les raccourcis et l'écoute de Ctrl sont suspendus.
- `popover.rs` : panneau de dictée (fenêtre sans bordure, verre dépoli, se ferme au clic ailleurs) : trois boutons de langue, réglages, `cycle_target` pour changer de langue sans ouvrir le panneau.
- `pastille.rs` : pastille toujours visible (repos, micro en préparation, écoute avec niveau de la voix, traitement avec barre d'attente estimée, collé, erreur). Fenêtre transparente, non focalisable (`_setPreventsActivation:` d'AppKit, testé avant usage), déplaçable, place retenue seulement pendant un vrai glisser. Rust pilote la page par `eval` de `window.__pastille`.
- `hud.rs` : encart façon HUD volume macOS (matériau `HudWindow`), 1,1 s au bas de l'écran du curseur ; un compteur de génération évite qu'une fermeture programmée ne ferme un affichage plus récent.
- `son.rs` : baisse du son pendant la dictée (réglage `son_dictee` : `baisser` au quart, `couper`, `garder`), par AppleScript. Volume noté dans `settings.json` avant la baisse, rendu au relâchement, à l'abandon, à la fermeture et au démarrage suivant s'il est resté noté.
- `lexique.rs` : mots à corriger (`lexique.txt` dans le dossier de données), une correction par mot, commentaires et ordre gardés.

## Décisions prises

- **Tout en local.** Ollama ne lit pas l'audio : la transcription est un second moteur, embarqué dans l'app plutôt qu'un service à lancer à côté.
- **Parakeet plutôt que Whisper ou Canary.** Parakeet v3 : 0,25 s / 0,6 s / 1,4 s pour 4 / 11 / 29 s de parole sur M1 Max, bon en français. Canary 1B v2 essayé : cinq fois plus lent, et forcé en français il francise les mots anglais (« Miro » devient « le bureau »). L'accélérateur Apple (CoreML) essayé : sept fois plus lent et plantage, écarté.
- **Aya Expanse 8B pour la dictée.** Comparé à Gemma 3 4B sur des phrases familières et des heures : Gemma faussait le sens en espagnol et changeait 14 h en 16 h ; Aya garde le sens, les heures et le tutoiement, pour 0,3 s de plus. `qwen3:4b` écarté (il « réfléchit » 17 s). Contrepartie : licence non commerciale (CC-BY-NC), d'où `mistral:latest` conseillé en usage professionnel.
- **Ctrl seul, maintenu, pour dicter.** Event tap actif plutôt qu'en écoute seule : l'actif ne demande que l'Accessibilité (déjà nécessaire pour coller), l'écoute seule demanderait en plus la « surveillance du clavier ». Délai de 200 ms pour distinguer une dictée d'un raccourci.
- **Raccourci dédié pour traduire une sélection** (Cmd+Maj+T), plutôt que le double Cmd+C de DeepL, qui demanderait la permission « Surveillance des entrées ».
- **Deux permissions seulement** : Accessibilité (Cmd+C, Cmd+V, touche Ctrl) et Micro.
- **Pastille visible au repos**, petite : elle montre l'état de la dictée, sert à changer de langue et à ouvrir le traducteur. L'icône de la barre de menus peut être invisible sur un écran à encoche chargé.
- **Ctrl+Option+Flèches pour changer de langue** : Ctrl+Flèche seul sert à Mission Control, Option ou Cmd+Flèche à la navigation dans le texte.
- **Baisser le son plutôt que mettre en pause** : la touche lecture relancerait une musique arrêtée.
- **Micro fermé juste après chaque dictée** par défaut : gardé ouvert, un casque Bluetooth reste en mode appel (son dégradé, voyant allumé).
- **Signature ad hoc à identité fixe** (`scripts/installer.sh`) : sans elle, macOS retire les autorisations à chaque nouvelle version. Un certificat auto-signé demanderait de toucher aux réglages de sécurité du Mac.
- **Fenêtre du traducteur cachée au démarrage** ; la fermer la cache seulement.

## Pièges connus

- **Accessibilité cochée mais refusée** après une nouvelle construction signée autrement : `tccutil reset Accessibility fr.monsieurguiz.traducteur-local`, une dictée, puis accepter la demande. Évité par la signature fixe.
- **Mode dev** : le binaire n'est pas signé, l'autorisation Accessibilité peut sauter à chaque recompilation, et c'est le terminal qui porte la permission Micro.
- **Évènement Tauri perdu** : un `emit` vers une page chargée mais pas encore à l'écoute part dans le vide sans erreur. Pour la pastille et le HUD, Rust appelle directement une fonction JS déjà en place (`WebviewWindow::eval`).
- **Micro Bluetooth lent** : jusqu'à 3 s à l'ouverture. Attente portée à 15 s ; la pastille passe au rouge quand le son arrive vraiment.
- **Faux relâchement** avec la répétition du clavier : l'arrêt attend 200 ms qu'aucun nouvel appui n'arrive.
- **Autre outil de dictée lancé** (Superwhisper...) : il capte ses propres raccourcis, peut baisser le volume et le laisser à zéro. N'en garder qu'un.
- **Modèles « abliterated » communautaires** (non censurés) : packaging aléatoire, certains génèrent sans fin. Les appels Ollama sont plafonnés pour qu'un modèle cassé ne fige jamais l'interface.
- **Langues** : Parakeet transcrit 25 langues européennes ; chinois et japonais seulement en sortie (sans espace ajoutée après la dictée).

## Pistes non faites

Auto-pivot français / anglais dans la fenêtre, traduction de documents, registre adapté à l'app active (Slack familier, mail formel), téléchargement du modèle de transcription depuis l'app, signature et notarisation Apple.
