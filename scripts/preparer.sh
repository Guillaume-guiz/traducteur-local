#!/bin/bash
# Prépare les deux modèles d'IA dont l'app a besoin, sur ce Mac :
#   1. le modèle de transcription (Parakeet v3, 480 Mo), pour la dictée ;
#   2. un modèle de traduction dans Ollama (aya-expanse:8b, 5 Go par défaut).
#
# Utile seul si l'app est déjà installée (version téléchargée toute faite),
# appelé automatiquement par scripts/installer.sh sinon.
#
# Usage : scripts/preparer.sh                 modèle de traduction par défaut
#         scripts/preparer.sh mistral:latest  un autre modèle Ollama
set -euo pipefail

ID="fr.monsieurguiz.traducteur-local"
MODELES="$HOME/Library/Application Support/$ID/models"
PARAKEET="parakeet-tdt-0.6b-v3-int8"
PARAKEET_URL="https://blob.handy.computer/parakeet-v3-int8.tar.gz"
LLM="${1:-aya-expanse:8b}"

ok()   { printf "  \033[32mok\033[0m  %s\n" "$1"; }
info() { printf "\n\033[1m%s\033[0m\n" "$1"; }
stop() { printf "\n\033[31mArrêt :\033[0m %s\n" "$1" >&2; exit 1; }

# 1. Modèle de transcription
info "1/2 Modèle de transcription (Parakeet v3)"
if [ -f "$MODELES/$PARAKEET/encoder-model.int8.onnx" ]; then
  ok "déjà présent"
else
  mkdir -p "$MODELES"
  echo "  Téléchargement de 480 Mo, une à quelques minutes..."
  curl -fL --progress-bar "$PARAKEET_URL" | tar xzf - -C "$MODELES" \
    || stop "téléchargement interrompu. Relance le script, il reprend à zéro proprement."
  [ -f "$MODELES/$PARAKEET/encoder-model.int8.onnx" ] || stop "archive inattendue, modèle introuvable après extraction."
  ok "installé dans $MODELES"
fi

# 2. Ollama et modèle de traduction
info "2/2 Modèle de traduction ($LLM) dans Ollama"
if ! command -v ollama >/dev/null 2>&1 && [ ! -d /Applications/Ollama.app ]; then
  stop "Ollama n'est pas installé. Télécharge-le sur https://ollama.com/download, ouvre-le une fois, puis relance ce script."
fi
if ! curl -sf http://localhost:11434/api/tags >/dev/null; then
  echo "  Ollama ne répond pas, je le lance..."
  open -a Ollama 2>/dev/null || (ollama serve >/dev/null 2>&1 &)
  for _ in $(seq 1 30); do curl -sf http://localhost:11434/api/tags >/dev/null && break; sleep 1; done
  curl -sf http://localhost:11434/api/tags >/dev/null || stop "Ollama ne démarre pas. Ouvre l'app Ollama à la main, puis relance."
fi
ok "Ollama répond"
if curl -sf http://localhost:11434/api/tags | grep -q "\"name\":\"$LLM\""; then
  ok "$LLM déjà installé"
else
  echo "  Téléchargement de $LLM (plusieurs Go, plusieurs minutes)..."
  ollama pull "$LLM" || stop "échec du téléchargement de $LLM."
  ok "$LLM installé"
fi

info "Modèles prêts."
