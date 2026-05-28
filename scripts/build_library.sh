#!/usr/bin/env bash
# build_library.sh — Download, convert, metadata-fetch, and link a Spotify playlist
# Usage: ./build_library.sh <playlist_url> [existing_songs.json]

set -uo pipefail  # no -e: we handle errors per-command manually

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLAYLIST_URL="${1:-}"
EXISTING_FILE="${2:-}"
SONGS_FILE="${SCRIPT_DIR}/songs.json"
MP3_DIR="${SCRIPT_DIR}/mp3"
WAV_DIR="${SCRIPT_DIR}/data/wav"
TMP_TRACKS=$(mktemp /tmp/tracks_XXXXXX.ndjson)

die() { echo "ERROR: $*" >&2; rm -f "$TMP_TRACKS"; exit 1; }
log() { echo "$*" >&2; }

trap 'rm -f "$TMP_TRACKS"' EXIT

# ── Args & tools ──────────────────────────────────────────────────────────────

[[ -z "$PLAYLIST_URL" ]] && die "Usage: $0 <playlist_url> [existing_songs.json]"
[[ -n "$EXISTING_FILE" && ! -f "$EXISTING_FILE" ]] && die "File not found: $EXISTING_FILE"

for tool in spotify-dl ffmpeg fzf jq; do
  command -v "$tool" &>/dev/null || die "'$tool' is required but not installed."
done

[[ -f "${SCRIPT_DIR}/spotify_playlist.sh" ]] || die "spotify_playlist.sh not found in $SCRIPT_DIR"
[[ -f "${SCRIPT_DIR}/tracks_format.sh"    ]] || die "tracks_format.sh not found in $SCRIPT_DIR"

mkdir -p "$MP3_DIR" "$WAV_DIR"

# ── Step 1: Download mp3s ─────────────────────────────────────────────────────

log ""
log "==> [1/4] Downloading playlist..."
spotify-dl -f mp3 -d "$MP3_DIR" "$PLAYLIST_URL" || log "  (spotify-dl exited non-zero — some tracks may have been skipped)"

# ── Step 2: Convert to wav ────────────────────────────────────────────────────

log ""
log "==> [2/4] Converting MP3s to WAV..."

CONVERTED=0
SKIPPED=0
FAILED=0

while IFS= read -r mp3; do
  name=$(basename "$mp3" .mp3)
  wav="${WAV_DIR}/${name}.wav"
  if [[ -f "$wav" ]]; then
    log "  Already exists, skipping: $name"
    (( SKIPPED++ )) || true
  else
    log "  Converting: $name"
    if ffmpeg -i "$mp3" -y "$wav" 2>/dev/null; then
      (( CONVERTED++ )) || true
    else
      log "  FAILED to convert: $name"
      (( FAILED++ )) || true
    fi
  fi
done < <(find "$MP3_DIR" -name "*.mp3" | sort)

log "  Conversion complete: $CONVERTED converted, $SKIPPED skipped, $FAILED failed"

# ── Step 3: Fetch & format metadata ──────────────────────────────────────────

log ""
log "==> [3/4] Fetching playlist metadata from Spotify..."

if [[ -n "$EXISTING_FILE" && -f "$EXISTING_FILE" ]]; then
  cat "$EXISTING_FILE" > "$TMP_TRACKS"
fi

bash "${SCRIPT_DIR}/spotify_playlist.sh" "$PLAYLIST_URL" \
  | bash "${SCRIPT_DIR}/tracks_format.sh" ${EXISTING_FILE:+"$EXISTING_FILE"} \
  >> "$TMP_TRACKS" \
  || die "Failed to fetch/format playlist metadata."

TOTAL=$(wc -l < "$TMP_TRACKS" | tr -d ' ')
log "  $TOTAL total tracks loaded."

# ── Step 4: Interactive file linking ─────────────────────────────────────────

log ""
log "==> [4/4] Linking WAV files to track entries..."
log "    Type to fuzzy-search | Enter to confirm | Esc to skip"
log ""

while IFS= read -r wav; do
  wav_basename=$(basename "$wav" .wav)
  rel_path="./data/wav/$(basename "$wav")"

  # Skip if already linked
  if grep -qF "\"$rel_path\"" "$TMP_TRACKS" 2>/dev/null; then
    log "  Already linked: $wav_basename"
    continue
  fi

  # Build candidate list from unlinked tracks: plain "Artist — Name" for clean searching
  CANDIDATES=$(jq -r 'select(.filename == null) | "\(.artist) — \(.name)"' "$TMP_TRACKS")

  if [[ -z "$CANDIDATES" ]]; then
    log "  No unlinked tracks remaining."
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

  # Look up ID by matching the selected label
  TRACK_ID=$(jq -r --arg label "$CHOICE" \
    'select(.filename == null) | select("\(.artist) — \(.name)" == $label) | .id' \
    "$TMP_TRACKS" | head -1)

  if [[ -z "$TRACK_ID" ]]; then
    log "  Could not find ID for: $CHOICE — skipping"
    continue
  fi

  TRACK_LABEL="$CHOICE"


  # Patch filename in temp file
  TMP2=$(mktemp /tmp/tracks_XXXXXX.ndjson)
  jq -c --argjson id "$TRACK_ID" --arg path "$rel_path" \
    'if .id == $id then .filename = $path else . end' \
    "$TMP_TRACKS" > "$TMP2" && mv "$TMP2" "$TMP_TRACKS"

  log "  Linked: [$TRACK_ID] $TRACK_LABEL"

done < <(find "$WAV_DIR" -name "*.wav" | sort)

# ── Save songs.json ───────────────────────────────────────────────────────────

cp "$TMP_TRACKS" "$SONGS_FILE"

LINKED=$(jq -r 'select(.filename != null) | .id' "$SONGS_FILE" | wc -l | tr -d ' ')
UNLINKED=$(jq -r 'select(.filename == null) | .id' "$SONGS_FILE" | wc -l | tr -d ' ')

log ""
log "==> Done. Saved to $SONGS_FILE"
log "    Total: $TOTAL | Linked: $LINKED | Unlinked: $UNLINKED"
