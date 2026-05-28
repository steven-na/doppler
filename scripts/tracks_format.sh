#!/usr/bin/env bash
# tracks_format.sh — Convert Spotify playlist JSON to newline-delimited track records
# Usage:
#   ./spotify_playlist.sh <url> | ./tracks_format.sh
#   ./spotify_playlist.sh <url> | ./tracks_format.sh existing_tracks.json

set -euo pipefail

die() { echo "ERROR: $*" >&2; exit 1; }

command -v jq &>/dev/null || die "'jq' is required but not installed."

# ── Find starting ID and load existing tracks ─────────────────────────────────

NEXT_ID=1
declare -A EXISTING_KEYS  # "cleaned_name|artist" -> 1

EXISTING_FILE="${1:-}"
if [[ -n "$EXISTING_FILE" ]]; then
  [[ -f "$EXISTING_FILE" ]] || die "File not found: $EXISTING_FILE"

  MAX_ID=$(jq -s '[.[].id] | max // 0' "$EXISTING_FILE")
  NEXT_ID=$(( MAX_ID + 1 ))

  # Build a lookup of "name|artist" from existing cleaned records
  while IFS= read -r entry; do
    EXISTING_KEYS["$entry"]=1
  done < <(jq -rs '.[] | "\(.name)|\(.artist)"' "$EXISTING_FILE")
fi

# ── Read playlist JSON from stdin ─────────────────────────────────────────────

INPUT=$(cat)
[[ -z "$INPUT" ]] && die "No input received on stdin."

# ── Strip remaster/reissue/deluxe suffixes ────────────────────────────────────
# Handles: "Song - 2024 Remaster"  "Song (Remastered)"  "Song [2011 Remaster]"
#          "Song - Deluxe"         "Song (Deluxe Edition)"  "Song (Anniversary Edition)"

clean() {
  echo "$1" | sed \
    -e 's/ [-–—] [0-9]*[[:space:]]*[Rr]e[Mm]aster[^)]*$//' \
    -e 's/ [-–—] [0-9]*[[:space:]]*[Rr]emastered[^)]*$//' \
    -e 's/ ([0-9]*[[:space:]]*[Rr]e[Mm]aster[^)]*)//' \
    -e 's/ ([0-9]*[[:space:]]*[Rr]emastered[^)]*)//' \
    -e 's/ \[[0-9]*[[:space:]]*[Rr]e[Mm]aster[^]]*\]//' \
    -e 's/ [-–—] [Dd]eluxe[^)]*$//' \
    -e 's/ ([Dd]eluxe[^)]*)//' \
    -e 's/ ([Ee]xpanded[^)]*)//' \
    -e 's/ ([Ss]pecial[^)]*[Ee]dition[^)]*)//' \
    -e 's/ ([0-9]*[a-z]*[[:space:]]*[Aa]nniversary[^)]*)//' \
    -e 's/:[^:]*[Aa]nniversary[^:]*$//' \
    -e 's/[[:space:]]*$//'
}

# ── Emit one JSON record per line, skipping duplicates ────────────────────────

SKIPPED=0

while IFS= read -r line; do
  name=$(echo "$line"   | jq -r '.name')
  artist=$(echo "$line" | jq -r '.artist')
  album=$(echo "$line"  | jq -r '.album')
  dur=$(echo "$line"    | jq    '.duration_seconds')
  id=$(echo "$line"     | jq    '.id')

  clean_name=$(clean "$name")
  clean_album=$(clean "$album")
  key="${clean_name}|${artist}"

  if [[ -n "${EXISTING_KEYS[$key]+_}" ]]; then
    echo "Skipping duplicate: $clean_name — $artist" >&2
    (( SKIPPED++ )) || true
    # Don't consume this ID
    continue
  fi

  jq -cn \
    --arg    name   "$clean_name" \
    --arg    artist "$artist" \
    --arg    album  "$clean_album" \
    --argjson dur   "$dur" \
    --argjson id    "$id" \
    '{name:$name, artist:$artist, album:$album, duration:$dur, filename:null, id:$id}'

done < <(echo "$INPUT" | jq -c --argjson start "$NEXT_ID" '
  .tracks | to_entries[] |
  {
    name:             .value.name,
    artist:           .value.artist,
    album:            .value.album,
    duration_seconds: .value.duration_seconds,
    id:               ($start + .key)
  }
')

if [[ $SKIPPED -gt 0 ]]; then echo "Skipped $SKIPPED duplicate(s)." >&2; fi
