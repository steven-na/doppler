#!/usr/bin/env bash
# link_files.sh — Interactively link wav files to entries in songs.json
# Usage: ./link_files.sh <wav_dir> <songs.json>

set -uo pipefail

die() { echo "ERROR: $*" >&2; exit 1; }
log() { echo "$*" >&2; }

WAV_DIR="${1:-}"
SONGS_FILE="${2:-}"

[[ -z "$WAV_DIR"    ]] && die "Usage: $0 <wav_dir> <songs.json>"
[[ -z "$SONGS_FILE" ]] && die "Usage: $0 <wav_dir> <songs.json>"
[[ -d "$WAV_DIR"    ]] || die "Directory not found: $WAV_DIR"
[[ -f "$SONGS_FILE" ]] || die "File not found: $SONGS_FILE"

for tool in fzf jq; do
  command -v "$tool" &>/dev/null || die "'$tool' is required but not installed."
done

TMP=$(mktemp /tmp/songs_XXXXXX.ndjson)
trap 'rm -f "$TMP"' EXIT
cp "$SONGS_FILE" "$TMP"

log "Type to search | Enter to confirm | Esc to skip"
log ""

while IFS= read -r wav; do
  wav_basename=$(basename "$wav"); wav_basename="${wav_basename%.*}"
  rel_path="${WAV_DIR%/}/$(basename "$wav")"

  # Skip if any entry already has a filename whose basename (sans extension) matches
  if jq -e --arg base "$wav_basename"     'select(.filename != null) | select((.filename | gsub(".*/"; "") | gsub("\\.[^.]+$"; "")) == $base)'     "$TMP" > /dev/null 2>&1; then
    log "Already linked: $wav_basename"
    continue
  fi

  CANDIDATES=$(jq -r 'select(.filename == null) | "\(.artist) — \(.name)"' "$TMP")

  if [[ -z "$CANDIDATES" ]]; then
    log "No unlinked tracks remaining."
    break
  fi

  CHOICE=$(
    echo "$CANDIDATES" \
    | fzf \
        --query="${wav_basename// - / }" \
        --prompt="  '$wav_basename' → " \
        --height=50% \
        --layout=reverse \
        --info=inline \
        --bind='esc:abort' \
      2>/dev/tty \
    || true
  )

  if [[ -z "$CHOICE" ]]; then
    log "  Skipped: $wav_basename"
    continue
  fi

  TRACK_ID=$(jq -r --arg label "$CHOICE" \
    'select(.filename == null) | select("\(.artist) — \(.name)" == $label) | .id' \
    "$TMP" | head -1)

  if [[ -z "$TRACK_ID" ]]; then
    log "  Could not find ID for: $CHOICE — skipping"
    continue
  fi

  TMP2=$(mktemp /tmp/songs_XXXXXX.ndjson)
  jq -c --argjson id "$TRACK_ID" --arg path "$rel_path" \
    'if .id == $id then .filename = $path else . end' \
    "$TMP" > "$TMP2" && mv "$TMP2" "$TMP"

  log "  Linked: [$TRACK_ID] $CHOICE"

done < <(find "$WAV_DIR" -name "*.wav" -o -name "*.mp3" | sort)

cp "$TMP" "$SONGS_FILE"

LINKED=$(jq -r 'select(.filename != null) | .id' "$SONGS_FILE" | wc -l | tr -d ' ')
UNLINKED=$(jq -r 'select(.filename == null) | .id' "$SONGS_FILE" | wc -l | tr -d ' ')
log ""
log "Done. Linked: $LINKED | Unlinked: $UNLINKED"
