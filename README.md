# Traducteur local

Une app macOS de traduction et de dictée qui fonctionne entièrement sur ton Mac. Aucun texte, aucune voix ne part sur internet : la traduction passe par un modèle d'IA installé chez toi (Ollama), la transcription de la voix aussi (Parakeet). Pensée comme une alternative à DeepL et aux outils de dictée en ligne.

Version 1.0.0, figée le 2 octobre 2026.

## Ce que fait l'app

**1. Une fenêtre de traduction**, comme DeepL : tu tapes ou colles un texte à gauche, la traduction s'affiche à droite au fil de la frappe.
- Langue d'origine détectée toute seule, langue cible au choix.
- Trois registres : Neutre, Formel, Familier.
- Reformuler la traduction : améliorer le style, plus formel, plus chaleureux, raccourcir, développer.
- Variantes : plusieurs traductions possibles d'une même phrase.
- Choix du modèle d'IA, et installation de nouveaux modèles depuis l'app.

**2. Traduire une sélection, depuis n'importe quelle app.** Tu sélectionnes un texte (dans un mail, Slack, un document), tu appuies sur Cmd+Maj+T : la fenêtre s'ouvre avec le texte déjà traduit. Le bouton Insérer remplace ta sélection par la traduction.

**3. La dictée, traduite ou non.** Tu maintiens la touche Ctrl, tu parles, tu relâches : le texte s'écrit là où se trouve ton curseur, dans n'importe quelle app. Tu peux le garder tel que tu l'as dit, ou le faire traduire à la volée (anglais, espagnol, allemand, italien, portugais, chinois, japonais).
- Une petite pastille flottante montre où en est la dictée : au repos, elle écoute (avec le niveau de ta voix), elle traduit, c'est collé.
- Le son du Mac baisse pendant que tu parles, puis revient.
- Une liste de mots à corriger, pour les noms propres que la dictée écrit de travers.

## Ce qu'il te faut

- Un Mac avec une puce Apple (M1, M2, M3, M4...). Un Mac Intel peut fonctionner, mais lentement, et n'a pas été testé.
- macOS 13 (Ventura) ou plus récent.
- 8 Go de mémoire au minimum, 16 Go conseillés.
- Environ 7 Go d'espace disque libre pour les modèles d'IA (5 Go pour la traduction, 700 Mo pour la dictée), plus 4 Go si tu construis l'app toi-même.
- [Ollama](https://ollama.com/download), gratuit : c'est lui qui fait tourner le modèle de traduction.

## Installation

Deux façons de faire. La première est la plus simple ; la seconde te permet de modifier l'app.

### Option A : télécharger l'app toute prête

1. Installe [Ollama](https://ollama.com/download) et ouvre-le une fois.
2. Télécharge `Traducteur-local-1.0.0-mac.zip` dans la page [Releases](https://github.com/Guillaume-guiz/traducteur-local/releases) du dépôt, décompresse-le et glisse `Traducteur local.app` dans ton dossier Applications.
3. L'app n'est pas signée par un compte développeur Apple, donc macOS la bloque au premier lancement (« l'app est endommagée » ou « développeur non identifié »). Ouvre le Terminal et lance :
   ```
   xattr -dr com.apple.quarantine "/Applications/Traducteur local.app"
   ```
4. Toujours dans le Terminal, télécharge les deux modèles d'IA (une seule fois, environ 6 Go) :
   ```
   curl -fsSL https://raw.githubusercontent.com/Guillaume-guiz/traducteur-local/main/scripts/preparer.sh | bash
   ```
5. Ouvre `Traducteur local` depuis le dossier Applications, puis passe à « Premier lancement » plus bas.

### Option B : construire l'app depuis le code source

Il te faut en plus les outils de développement :
- les outils Apple : `xcode-select --install`
- Rust 1.88 ou plus récent : `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Node.js 18 ou plus récent : depuis [nodejs.org](https://nodejs.org) ou `brew install node`

Puis :

```
git clone https://github.com/Guillaume-guiz/traducteur-local.git
cd traducteur-local
scripts/installer.sh
```

Le script vérifie les outils et te dit quoi installer s'il en manque un, télécharge les modèles d'IA, construit l'app (5 à 15 minutes la première fois), la signe, la copie dans Applications et la lance. Pour mettre à jour après une modification du code, relance le même script.

## Premier lancement

macOS demande deux autorisations, une seule fois :

- **Micro**, à la première dictée. Sans lui, la dictée n'entend rien.
- **Accessibilité**, à la première dictée ou au premier « Insérer ». Elle permet à l'app de coller le texte là où est ton curseur, de copier ta sélection, et de reconnaître l'appui sur Ctrl. Si macOS ne la demande pas : Réglages Système, Confidentialité et sécurité, Accessibilité, puis active `Traducteur local`.

L'app vit dans la barre de menus (icône en haut de l'écran) et dans la pastille flottante. Sa fenêtre est cachée au démarrage : on l'ouvre quand on en a besoin.

## Mode d'emploi

### La fenêtre de traduction

Pour l'ouvrir : clic sur le bouton d'agrandissement de la pastille, bouton « Traducteur » du panneau de la barre de menus, ou icône du Dock.

- Tape ou colle ton texte à gauche. Cmd+Entrée relance la traduction.
- Choisis la langue cible en haut, et le registre (Neutre, Formel, Familier).
- Sous la traduction : Reformuler, Variantes, Copier, Insérer.
- Le menu déroulant en haut à droite choisit le modèle d'IA. La roue dentée ouvre les réglages : le raccourci global, et l'onglet Modèles, qui affiche la configuration de ton Mac, recommande des modèles adaptés et les installe en un clic.

Fermer la fenêtre la cache seulement : l'app continue de tourner.

### Traduire une sélection

1. Sélectionne un texte dans n'importe quelle app.
2. Appuie sur Cmd+Maj+T. La fenêtre s'ouvre avec le texte et sa traduction.
3. Clique sur Insérer : la traduction remplace ta sélection, dans l'app d'origine.

Le raccourci se change dans la roue dentée de la fenêtre.

### Dicter

1. Place ton curseur là où tu veux écrire (un mail, un message, un document).
2. Maintiens la touche Ctrl, attends que la pastille passe au rouge, et parle.
3. Relâche Ctrl : le texte s'écrit à la place du curseur, traduit ou non selon la langue choisie.

Bon à savoir :
- Ctrl suivi d'une autre touche (Ctrl+C, par exemple) ne déclenche pas la dictée : tes raccourcis habituels fonctionnent toujours.
- Pendant que tu parles, un clic place le curseur là où le texte sera collé.
- Tu peux dicter longtemps : la transcription avance pendant que tu parles, par morceaux, pour que le texte arrive vite au relâchement.
- La dictée comprend 25 langues européennes en entrée (français, anglais, espagnol, allemand, italien...). Le chinois et le japonais ne sont disponibles qu'en sortie.

### Choisir la langue de la dictée

Trois langues sont à portée de main. Par défaut : Français, Anglais, Espagnol.

- **Français** colle le texte tel que tu l'as dit, sans passer par le modèle de traduction. C'est le plus rapide. Si tu dictes en anglais, il sort en anglais.
- **Français nettoyé** passe le texte par le modèle pour retirer les hésitations (« euh », répétitions).
- Les autres langues traduisent ce que tu as dit.

Pour changer de langue :
- clic sur la langue affichée dans la pastille (elle passe à la suivante) ;
- Ctrl+Option+Flèche droite ou Ctrl+Option+Flèche gauche, depuis n'importe quelle app ; un petit encart au bas de l'écran confirme la langue choisie ;
- clic sur l'icône de la barre de menus, puis sur l'une des trois langues.

### La pastille

Elle reste visible par-dessus les autres fenêtres, sans jamais prendre la main sur l'app où tu écris.
- Déplace-la à la souris, elle garde sa place.
- Clic sur la langue : langue suivante.
- Bouton d'agrandissement : ouvre la fenêtre de traduction avec ta dernière dictée (ce que tu as dit à gauche, ce qui a été collé à droite).
- Clic droit : le panneau de la dictée.

### Le panneau de la dictée et ses réglages

Clic sur l'icône de la barre de menus (ou clic droit sur la pastille). Le bouton Réglages donne accès à :
- **les trois langues** proposées dans le panneau et la pastille ;
- **les raccourcis** : dicter (Ctrl seul, ou une combinaison à maintenir), langue suivante, langue précédente, traduire la sélection. Clique sur un raccourci puis tape la nouvelle combinaison ; Échap annule, Retour arrière remet celui d'origine ;
- **le son pendant la dictée** : baissé au quart (par défaut), coupé, ou gardé ;
- **le micro gardé ouvert** après une dictée, pour que la suivante démarre plus vite. Fermé tout de suite par défaut : avec un casque Bluetooth, le garder ouvert laisse le casque en mode appel ;
- **les mots à corriger** : quand la dictée écrit un mot de travers (« Monsieur Guise » au lieu de « Monsieur Guiz »), indique ce qu'elle écrit et ce que tu veux. Pris en compte dès la dictée suivante. Chaque variante demande sa ligne.

Le clic droit sur l'icône de la barre de menus ouvre le menu de l'app : afficher le traducteur, modifier le raccourci, quitter.

### Changer de modèle d'IA

Le modèle installé par défaut, `aya-expanse:8b` (5 Go), traduit bien une vingtaine de langues et sert à la fois à la fenêtre et à la dictée. Sa licence interdit l'usage commercial : pour un usage professionnel, prends plutôt `mistral:latest`. Les choix, dans l'onglet Modèles des réglages de la fenêtre :
- `mistral:latest` (4 Go, licence Apache 2.0) : rapide, ne refuse jamais de traduire un contenu cru ;
- `gemma3:4b` (3 Go) : plus léger, pour les Mac à 8 Go de mémoire, moins fidèle en allemand et en espagnol ;
- des modèles plus gros (Gemma 2 27B, Aya Expanse 32B) pour une meilleure qualité, si ton Mac a 32 Go de mémoire ou plus.

Le modèle de la dictée se règle à part, dans le fichier de réglages (voir plus bas, clé `dictation_model`). Sans ce réglage, la dictée utilise `aya-expanse:8b`, ou à défaut le modèle choisi dans la fenêtre.

Pour installer un autre modèle dès le départ : `scripts/preparer.sh mistral:latest`.

## Où sont rangées les choses

Tout est dans `~/Library/Application Support/fr.monsieurguiz.traducteur-local/` :
- `settings.json` : les réglages (raccourcis, langues, modèles, place de la pastille) ;
- `lexique.txt` : les mots à corriger, une correction par ligne, `ce qui sort => ce qu'on veut`. Modifiable à la main ;
- `models/` : le modèle de transcription ;
- `dictee.log` : le journal des durées de chaque dictée (jamais le texte dicté), utile pour comprendre une lenteur.

Les modèles de traduction sont gérés par Ollama, dans `~/.ollama/`.

## Dépannage

**« Ollama hors ligne » dans la fenêtre.** Ouvre l'app Ollama (elle se place dans la barre de menus), puis rouvre la fenêtre.

**« Aucun modèle ».** Ollama tourne mais n'a aucun modèle : installe-en un depuis l'onglet Modèles des réglages, ou lance `ollama pull aya-expanse:8b` dans le Terminal.

**La dictée affiche « Modèle de transcription absent ».** Le modèle de la dictée n'est pas téléchargé : lance `scripts/preparer.sh` (ou la commande `curl` de l'option A).

**Ctrl ne déclenche rien, ou le texte ne se colle pas.** L'autorisation Accessibilité manque. Si elle est cochée mais ne marche pas (fréquent après une mise à jour de l'app), remets-la à zéro dans le Terminal :
```
tccutil reset Accessibility fr.monsieurguiz.traducteur-local
```
puis fais une dictée et accepte la demande de macOS.

**La pastille dit « Rien entendu ».** Le micro n'a rien capté : vérifie l'autorisation Micro (Réglages Système, Confidentialité et sécurité, Micro) et le micro choisi dans les réglages Son du Mac.

**Le premier mot est coupé avec un casque Bluetooth.** Un casque Bluetooth met une à trois secondes à ouvrir son micro. Attends que la pastille passe au rouge avant de parler, ou garde le micro ouvert quelques minutes (réglages du panneau).

**Le son ne revient pas après une dictée.** L'app remet le volume qu'elle a noté avant de le baisser, même après un arrêt brutal (au lancement suivant). Si un autre outil de dictée tourne en même temps (Superwhisper, par exemple), il peut aussi baisser le son : n'en garde qu'un.

**L'icône n'apparaît pas dans la barre de menus.** Sur un écran à encoche, une barre de menus trop chargée masque les icônes en trop. La pastille et les raccourcis clavier font tout ce que fait l'icône.

**La traduction est lente ou fige.** Le premier appel charge le modèle en mémoire (quelques secondes). Si c'est lent à chaque fois, prends un modèle plus léger. Un appel ne peut pas bloquer l'app plus de deux minutes.

## Désinstaller

1. Quitte l'app (clic droit sur l'icône de la barre de menus, Quitter) et supprime `Traducteur local` du dossier Applications.
2. Supprime le dossier `~/Library/Application Support/fr.monsieurguiz.traducteur-local/`.
3. Retire les autorisations :
   ```
   tccutil reset Accessibility fr.monsieurguiz.traducteur-local
   tccutil reset Microphone fr.monsieurguiz.traducteur-local
   ```
4. Si tu ne te sers plus d'Ollama : `ollama rm aya-expanse:8b`, puis supprime l'app Ollama.

## Comment c'est construit

- **Tauri 2** : une app native macOS dont le moteur est écrit en Rust et l'interface en HTML, CSS et TypeScript.
- **Ollama** pour la traduction : l'app lui parle sur `http://localhost:11434`, en local.
- **Parakeet v3** (NVIDIA, version int8 au format ONNX) pour la transcription, embarqué dans l'app grâce à la bibliothèque `transcribe-rs`. Environ 0,2 seconde pour une phrase, 1,4 seconde pour 30 secondes de parole sur une puce M1 Max.
- **Les API de macOS** pour le reste : écoute de la touche Ctrl, simulation de Cmd+C et Cmd+V, réglage du volume, fenêtres flottantes.

Le détail (rôle de chaque fichier, décisions prises et pourquoi, pièges rencontrés) est dans [CLAUDE.md](CLAUDE.md).

Comment cette app a été construite avec Claude Code, étape par étape, et les garde-fous pour construire la sienne : [docs/construire-son-app-mac.md](docs/construire-son-app-mac.md). Ce fichier sert aussi de contexte à Claude Code : ouvre le dossier avec Claude Code et pose-lui tes questions sur le code, ou demande-lui une modification.

Pour lancer l'app en mode développement (rechargement automatique de l'interface) :
```
npm install
npm run tauri dev
```

## Limites connues

- macOS uniquement.
- Un modèle de traduction de 8 milliards de paramètres se trompe parfois : relis les chiffres et les heures d'un texte important.
- Sans compte développeur Apple, l'app n'est pas notarisée : d'où la commande `xattr` de l'option A.
- La dictée ne distingue pas plusieurs voix, et n'adapte pas son registre à l'app où tu écris.

## Licence et crédits

Code sous licence MIT (voir [LICENSE](LICENSE)), par Monsieur Guiz.

L'app s'appuie sur :
- [Ollama](https://ollama.com) et les modèles que tu choisis d'y installer, chacun sous sa propre licence (Aya Expanse : CC-BY-NC 4.0, usage non commercial ; Mistral 7B : Apache 2.0) ;
- le modèle [Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) de NVIDIA (CC-BY 4.0), dans sa version int8 distribuée par le projet [Handy](https://github.com/cjpais/Handy) ;
- [transcribe-rs](https://crates.io/crates/transcribe-rs), [Tauri](https://tauri.app), [cpal](https://crates.io/crates/cpal).
