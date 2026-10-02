#!/bin/bash
# Installe le Traducteur local à partir du code source, de bout en bout :
# vérifie les outils, prépare les modèles d'IA, construit l'app, la signe et
# la copie dans /Applications.
#
# Pourquoi la signature : sans compte développeur Apple, l'app est signée
# « ad hoc ». Une signature ad hoc ordinaire change à chaque construction, et
# macOS retire alors les autorisations Micro et Accessibilité accordées à la
# version précédente (tout en les affichant cochées). On signe donc avec une
# exigence fixe, « identifier fr.monsieurguiz.traducteur-local » : macOS
# reconnaît l'app d'une version à l'autre et garde les autorisations.
#
# Usage : scripts/installer.sh                 tout, puis lance l'app
#         scripts/installer.sh --sans-lancer   sans lancer l'app à la fin
#         scripts/installer.sh --sans-modeles  sans préparer les modèles
set -euo pipefail
cd "$(dirname "$0")/.."

ID="fr.monsieurguiz.traducteur-local"
NOM="Traducteur local"
APP="src-tauri/target/release/bundle/macos/$NOM.app"
LANCER=1
MODELES=1
for a in "$@"; do
  case "$a" in
    --sans-lancer) LANCER=0 ;;
    --sans-modeles) MODELES=0 ;;
    *) echo "Option inconnue : $a" >&2; exit 2 ;;
  esac
done

ok()   { printf "  \033[32mok\033[0m  %s\n" "$1"; }
info() { printf "\n\033[1m%s\033[0m\n" "$1"; }
stop() { printf "\n\033[31mArrêt :\033[0m %s\n" "$1" >&2; exit 1; }
version_ok() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" = "$2" ]; }

info "Vérification des outils"
[ "$(uname -s)" = "Darwin" ] || stop "cette app ne fonctionne que sur macOS."
MACOS="$(sw_vers -productVersion)"
version_ok "$MACOS" "13.0" || stop "macOS 13 (Ventura) ou plus récent requis, tu as $MACOS."
ok "macOS $MACOS"
if [ "$(uname -m)" != "arm64" ]; then
  echo "  Attention : Mac Intel détecté. L'app est pensée pour les puces Apple (M1 et suivantes) ; elle peut marcher, mais lentement."
fi
xcode-select -p >/dev/null 2>&1 || stop "outils de développement Apple absents. Lance : xcode-select --install, puis relance ce script."
ok "outils de développement Apple"
command -v cargo >/dev/null 2>&1 || stop "Rust absent. Installe-le avec : curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh, ouvre un nouveau terminal, puis relance."
RUST="$(rustc --version | awk '{print $2}')"
version_ok "$RUST" "1.88.0" || stop "Rust 1.88 ou plus récent requis, tu as $RUST. Mets-le à jour : rustup update"
ok "Rust $RUST"
command -v npm >/dev/null 2>&1 || stop "Node.js absent. Installe la version LTS depuis https://nodejs.org (ou : brew install node), puis relance."
NODE="$(node -v | tr -d v)"
version_ok "$NODE" "18.0.0" || stop "Node.js 18 ou plus récent requis, tu as $NODE."
ok "Node.js $NODE"

if [ "$MODELES" = 1 ]; then
  bash scripts/preparer.sh
fi

info "Construction de l'app (5 à 15 minutes la première fois)"
npm ci --no-audit --no-fund
npm run tauri build -- --bundles app
[ -d "$APP" ] || stop "construction terminée sans app ($APP introuvable)."

info "Signature"
codesign --force --deep -s - --identifier "$ID" \
  -r="designated => identifier \"$ID\"" "$APP"
codesign -d -r- "$APP" 2>&1 | grep -q "designated => identifier \"$ID\"" \
  || stop "signature inattendue, installation annulée."
ok "signée ($ID)"

info "Installation dans /Applications"
osascript -e "quit app \"$NOM\"" 2>/dev/null || true
sleep 2
rm -rf "/Applications/$NOM.app"
ditto "$APP" "/Applications/$NOM.app"
ok "/Applications/$NOM.app"

if [ "$LANCER" = 1 ]; then
  open "/Applications/$NOM.app"
  info "C'est lancé. La pastille de dictée apparaît à l'écran ; macOS demandera le Micro et l'Accessibilité à la première dictée (voir le README)."
fi
