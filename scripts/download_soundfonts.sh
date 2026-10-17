#!/usr/bin/env bash
# scripts/download_soundfonts.sh
# Script de aprovisionamiento de SoundFonts oficiales para macOS / Linux en CI/CD
set -euo pipefail

TARGET_DIR="${1:-soundfonts}"
BASE_URL="https://github.com/Vladivostok214/ImpactSamplerStudio/releases/download/soundfonts-v1.0.0"

mkdir -p "$TARGET_DIR"

echo "=========================================================="
echo " ImpactSampler Studio - Aprovisionamiento de SoundFonts (macOS/Linux)"
echo "=========================================================="

# Formato: "RemoteName|LocalName|ExpectedHash"
SOUNDFONTS=(
  "Mellotron.sf2|Mellotron.sf2|F697AC18509B51061FE341CFA2D825DD221285882A4401A6A7121093081283F8"
  "Piano.-.Realtime.sf2|Piano - Realtime.sf2|3CC74C758618D58DCD67DB0A3BB51E73A6D7F78C7F312082645C695D388E66EE"
  "Rhodes.from.Dream.Piano.sf2|Rhodes (from Dream Piano).sf2|EA2733C334E382D60F5B02288302536ABDCF046EDE2B02907328E8BAB2F83B05"
  "Yamaha_RX7_Drums.sf2|Yamaha_RX7_Drums.sf2|E25B9E383FA8EDED889268446CA8F2D20C4E9D643503BAE0404C60CFA0C2EE11"
)

compute_sha256() {
  local file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print toupper($1)}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print toupper($1)}'
  else
    openssl dgst -sha256 "$file" | awk '{print toupper($NF)}'
  fi
}

for ENTRY in "${SOUNDFONTS[@]}"; do
  IFS='|' read -r REMOTE_NAME LOCAL_NAME EXPECTED_HASH <<< "$ENTRY"
  DEST="$TARGET_DIR/$LOCAL_NAME"

  NEEDS_DOWNLOAD=true
  if [ -f "$DEST" ]; then
    CURRENT_HASH=$(compute_sha256 "$DEST")
    if [ "$CURRENT_HASH" = "$EXPECTED_HASH" ]; then
      echo "Verificando archivo local existente: $LOCAL_NAME [OK - Checksum Verificado]"
      NEEDS_DOWNLOAD=false
    else
      echo "Checksum no coincide para $LOCAL_NAME, re-descargando..."
      rm -f "$DEST"
    fi
  fi

  if [ "$NEEDS_DOWNLOAD" = true ]; then
    URL="$BASE_URL/$REMOTE_NAME"
    echo "Descargando $LOCAL_NAME desde $URL..."
    curl -fSL --retry 3 --retry-delay 2 -o "$DEST" "$URL"

    COMPUTED_HASH=$(compute_sha256 "$DEST")
    if [ "$COMPUTED_HASH" = "$EXPECTED_HASH" ]; then
      echo " -> $LOCAL_NAME verificado exitosamente [SHA256 Match]"
    else
      echo "ERROR: Checksum invalido para $LOCAL_NAME! Esperado: $EXPECTED_HASH, Obtenido: $COMPUTED_HASH"
      exit 1
    fi
  fi
done

echo "Todos los SoundFonts de fabrica verificados e instalados correctamente en '$TARGET_DIR'."
