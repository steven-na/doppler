#!/usr/bin/env bash
# build_library.sh — Unified Doppler library builder.
#
# Downloads a Spotify playlist as mp3s, merges metadata into songs.json
# (without overwriting existing rows), links mp3 files to track entries
# (auto-linking when there is a single fuzzy match), bulk-fetches lyrics
# from LRCLIB, and lets you interactively fix each failed lookup.
#
# Usage:
#   ./build_library.sh [--playlist URL] [--songs FILE] [--lyrics FILE] [--mp3-dir DIR]
#
# Any missing input is prompted for with a default pre-filled.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

die() { echo "ERROR: $*" >&2; exit 1; }
log() { echo "$*" >&2; }

# ─── arg parsing ─────────────────────────────────────────────────────────────

PLAYLIST_URL=""
SONGS_FILE=""
LYRICS_FILE=""
MP3_DIR=""

usage() {
  cat <<EOF
Usage: $0 [--playlist URL] [--songs FILE] [--lyrics FILE] [--mp3-dir DIR]

  --playlist URL     Spotify playlist URL or ID
  --songs FILE       Existing songs.json to merge into (default: $SCRIPT_DIR/songs.json)
  --lyrics FILE      Existing lyrics.json to merge into (default: $SCRIPT_DIR/lyrics.json)
  --mp3-dir DIR      Output directory for mp3s          (default: $SCRIPT_DIR/mp3)
  -h, --help         Show this help and exit
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --playlist) PLAYLIST_URL="${2:-}"; shift 2 ;;
    --songs)    SONGS_FILE="${2:-}";   shift 2 ;;
    --lyrics)   LYRICS_FILE="${2:-}";  shift 2 ;;
    --mp3-dir)  MP3_DIR="${2:-}";      shift 2 ;;
    -h|--help)  usage; exit 0 ;;
    *) die "Unknown argument: $1" ;;
  esac
done

# ─── prerequisites ───────────────────────────────────────────────────────────

for tool in spotify-dl fzf jq curl python3; do
  command -v "$tool" &>/dev/null || die "'$tool' is required but not installed."
done

# ─── interactive prompts (always shown, defaults pre-filled) ─────────────────

prompt_with_default() {
  # prompt_with_default <varname> <prompt-label> <default>
  local __var="$1" label="$2" def="$3" reply=""
  while :; do
    read -e -p "$label " -i "$def" reply
    if [[ -n "$reply" ]]; then
      printf -v "$__var" '%s' "$reply"
      return
    fi
    log "  (cannot be empty)"
  done
}

log ""
log "==> Configuration"

if [[ -z "$PLAYLIST_URL" ]]; then
  prompt_with_default PLAYLIST_URL "Spotify playlist URL:" ""
fi
prompt_with_default SONGS_FILE  "songs.json path:       " "${SONGS_FILE:-$SCRIPT_DIR/songs.json}"
prompt_with_default LYRICS_FILE "lyrics.json path:      " "${LYRICS_FILE:-$SCRIPT_DIR/lyrics.json}"
prompt_with_default MP3_DIR     "mp3 output directory:  " "${MP3_DIR:-$SCRIPT_DIR/mp3}"

mkdir -p "$MP3_DIR" || die "Could not create $MP3_DIR"
mkdir -p "$(dirname "$SONGS_FILE")"  || die "Could not create parent of $SONGS_FILE"
mkdir -p "$(dirname "$LYRICS_FILE")" || die "Could not create parent of $LYRICS_FILE"

# ─── credentials ─────────────────────────────────────────────────────────────

CLIENT_ID="${SPOTIFY_CLIENT_ID:-}"
CLIENT_SECRET="${SPOTIFY_CLIENT_SECRET:-}"
if [[ -z "$CLIENT_ID" || -z "$CLIENT_SECRET" ]]; then
  log ""
  log "Spotify credentials not in environment."
  [[ -z "$CLIENT_ID"     ]] && read -rp "  SPOTIFY_CLIENT_ID:     " CLIENT_ID
  [[ -z "$CLIENT_SECRET" ]] && read -rp "  SPOTIFY_CLIENT_SECRET: " CLIENT_SECRET
fi
[[ -z "$CLIENT_ID" || -z "$CLIENT_SECRET" ]] && die "Spotify Client ID and Secret are required."

# ─── working copies ──────────────────────────────────────────────────────────

TMP_SONGS=$(mktemp /tmp/songs_XXXXXX.ndjson)
TMP_LYRICS=$(mktemp /tmp/lyrics_XXXXXX.ndjson)
TMP_RESP=$(mktemp /tmp/lrclib_XXXXXX.json)
trap 'rm -f "$TMP_SONGS" "$TMP_LYRICS" "$TMP_RESP"' EXIT

[[ -f "$SONGS_FILE"  ]] && cp "$SONGS_FILE"  "$TMP_SONGS"  || : > "$TMP_SONGS"
[[ -f "$LYRICS_FILE" ]] && cp "$LYRICS_FILE" "$TMP_LYRICS" || : > "$TMP_LYRICS"

SONGS_DIRTY=0
LYRICS_DIRTY=0

# ─── Phase 1: download mp3s ──────────────────────────────────────────────────

log ""
log "==> [1/6] Downloading playlist as mp3 → $MP3_DIR"
SPOTIFY_DL_MAX_TRIES=3
attempt=1
while :; do
  if spotify-dl -f mp3 -d "$MP3_DIR" "$PLAYLIST_URL"; then
    break
  fi
  if [[ $attempt -ge $SPOTIFY_DL_MAX_TRIES ]]; then
    log "  spotify-dl failed after $attempt attempts — continuing with whatever downloaded"
    break
  fi
  log "  spotify-dl failed (attempt $attempt/$SPOTIFY_DL_MAX_TRIES) — retrying in 5s..."
  sleep 5
  attempt=$(( attempt + 1 ))
done

# ─── Phase 2: fetch playlist metadata ────────────────────────────────────────

log ""
log "==> [2/6] Fetching playlist metadata from Spotify"

PLAYLIST_ID=$(echo "$PLAYLIST_URL" | sed -E 's|.*playlist/([A-Za-z0-9]+).*|\1|')
[[ -z "$PLAYLIST_ID" ]] && die "Could not parse playlist ID from: $PLAYLIST_URL"

AUTH=$(curl -s -X POST "https://accounts.spotify.com/api/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  --data-urlencode "grant_type=client_credentials" \
  --data-urlencode "client_id=$CLIENT_ID" \
  --data-urlencode "client_secret=$CLIENT_SECRET")
ACCESS_TOKEN=$(echo "$AUTH" | jq -r '.access_token // empty')
[[ -z "$ACCESS_TOKEN" ]] && die "Spotify auth failed: $AUTH"

META=$(curl -s "https://api.spotify.com/v1/playlists/${PLAYLIST_ID}?fields=name" \
  -H "Authorization: Bearer $ACCESS_TOKEN")
PLAYLIST_NAME=$(echo "$META" | jq -r '.name // "unknown"')
log "  Playlist: $PLAYLIST_NAME"

TRACKS_JSON="[]"
OFFSET=0
LIMIT=100
TOTAL=-1
while :; do
  RESP=$(curl -s -G "https://api.spotify.com/v1/playlists/${PLAYLIST_ID}/tracks" \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    --data-urlencode "offset=$OFFSET" \
    --data-urlencode "limit=$LIMIT" \
    --data-urlencode "fields=total,items(track(name,duration_ms,artists(name),album(name,release_date)))")
  ERR=$(echo "$RESP" | jq -r '.error.message // empty')
  [[ -n "$ERR" ]] && die "Spotify API: $ERR"
  [[ $TOTAL -eq -1 ]] && TOTAL=$(echo "$RESP" | jq '.total')

  PAGE=$(echo "$RESP" | jq '[
    .items[] | select(.track != null) | .track | {
      name:             .name,
      artist:           ([.artists[].name] | join(", ")),
      album:            .album.name,
      release_date:     .album.release_date,
      duration_seconds: (.duration_ms / 1000 | floor)
    }
  ]')
  TRACKS_JSON=$(jq -n --argjson acc "$TRACKS_JSON" --argjson p "$PAGE" '$acc + $p')

  OFFSET=$(( OFFSET + LIMIT ))
  log "  Fetched $(( OFFSET < TOTAL ? OFFSET : TOTAL )) / $TOTAL"
  [[ $OFFSET -ge $TOTAL ]] && break
done

# ─── Phase 3: merge into songs.json work-copy ────────────────────────────────

log ""
log "==> [3/6] Merging tracks into songs.json"

clean_title() {
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

NEXT_ID=1
declare -A EXISTING_KEYS
if [[ -s "$TMP_SONGS" ]]; then
  MAX_ID=$(jq -s '[.[].id] | max // 0' "$TMP_SONGS")
  NEXT_ID=$(( MAX_ID + 1 ))
  while IFS= read -r entry; do
    EXISTING_KEYS["$entry"]=1
  done < <(jq -rs '.[] | "\(.name)|\(.artist)"' "$TMP_SONGS")
fi

NEW_ROWS=0
SKIPPED_DUPES=0

while IFS= read -r raw; do
  name=$(  echo "$raw" | jq -r '.name')
  artist=$(echo "$raw" | jq -r '.artist')
  album=$( echo "$raw" | jq -r '.album')
  dur=$(   echo "$raw" | jq    '.duration_seconds')

  cname=$(clean_title "$name")
  calbum=$(clean_title "$album")
  key="${cname}|${artist}"

  if [[ -n "${EXISTING_KEYS[$key]+_}" ]]; then
    (( SKIPPED_DUPES++ )) || true
    continue
  fi

  jq -cn \
    --arg    name   "$cname" \
    --arg    artist "$artist" \
    --arg    album  "$calbum" \
    --argjson dur   "$dur" \
    --argjson id    "$NEXT_ID" \
    '{name:$name, artist:$artist, album:$album, duration:$dur, filename:null, id:$id}' \
    >> "$TMP_SONGS"

  EXISTING_KEYS["$key"]=1
  NEXT_ID=$(( NEXT_ID + 1 ))
  (( NEW_ROWS++ )) || true
done < <(echo "$TRACKS_JSON" | jq -c '.[]')

log "  New rows: $NEW_ROWS | Duplicates skipped: $SKIPPED_DUPES"
[[ $NEW_ROWS -gt 0 ]] && SONGS_DIRTY=1

# ─── Phase 4: link mp3 files (auto-link if exactly one match) ────────────────

log ""
log "==> [4/6] Linking mp3 files to track entries"
log "    Auto-linking when exactly one match; fzf for 0 or many."
log ""

while IFS= read -r mp3; do
  bn_full=$(basename "$mp3")
  bn="${bn_full%.mp3}"
  rel_path="$MP3_DIR/$bn_full"

  # Already linked?
  if jq -e --arg b "$bn_full" \
       'select(.filename != null) | select((.filename | gsub(".*/"; "")) == $b)' \
       "$TMP_SONGS" >/dev/null 2>&1; then
    log "  Already linked: $bn"
    continue
  fi

  CANDIDATES=$(jq -r 'select(.filename == null) | "\(.artist) — \(.name)"' "$TMP_SONGS")
  if [[ -z "$CANDIDATES" ]]; then
    log "  No unlinked tracks remain."
    break
  fi

  query="${bn// - / }"
  matches=$(printf '%s\n' "$CANDIDATES" | fzf --filter="$query" 2>/dev/null || true)
  match_count=0
  if [[ -n "$matches" ]]; then
    match_count=$(printf '%s\n' "$matches" | wc -l | tr -d ' ')
  fi

  CHOICE=""
  if [[ "$match_count" -eq 1 ]]; then
    CHOICE="$matches"
    log "  Auto-linked: $bn → $CHOICE"
  else
    CHOICE=$(
      printf '%s\n' "$CANDIDATES" | fzf \
        --query="$query" \
        --prompt="  '$bn' → " \
        --height=50% \
        --layout=reverse \
        --info=inline \
        --bind='esc:abort' \
        2>/dev/tty \
      || true
    )
    if [[ -z "$CHOICE" ]]; then
      log "  Skipped: $bn"
      continue
    fi
    log "  Linked: $bn → $CHOICE"
  fi

  TRACK_ID=$(jq -r --arg label "$CHOICE" \
    'select(.filename == null) | select("\(.artist) — \(.name)" == $label) | .id' \
    "$TMP_SONGS" | head -1)

  if [[ -z "$TRACK_ID" ]]; then
    log "    (could not resolve ID, skipping)"
    continue
  fi

  TMP2=$(mktemp /tmp/songs_XXXXXX.ndjson)
  jq -c --argjson id "$TRACK_ID" --arg path "$rel_path" \
    'if .id == $id then .filename = $path else . end' \
    "$TMP_SONGS" > "$TMP2" && mv "$TMP2" "$TMP_SONGS"

  SONGS_DIRTY=1
done < <(find "$MP3_DIR" -maxdepth 1 -name "*.mp3" | sort)

# ─── Phase 5: persist songs.json (only if changed) ───────────────────────────

log ""
if [[ $SONGS_DIRTY -eq 1 ]]; then
  cp "$TMP_SONGS" "$SONGS_FILE"
  LINKED=$(jq -r   'select(.filename != null) | .id' "$SONGS_FILE" | wc -l | tr -d ' ')
  UNLINKED=$(jq -r 'select(.filename == null) | .id' "$SONGS_FILE" | wc -l | tr -d ' ')
  TOTAL=$(wc -l < "$SONGS_FILE" | tr -d ' ')
  log "==> Wrote $SONGS_FILE  (total: $TOTAL | linked: $LINKED | unlinked: $UNLINKED)"
else
  log "==> songs.json unchanged — not writing"
fi

# ─── Phase 6: auto-fetch lyrics ──────────────────────────────────────────────

log ""
log "==> [5/6] Fetching lyrics from LRCLIB"

urlencode() {
  python3 -c "import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))" "$1"
}

parse_response() {
  # prints compact JSON {is_synced,timings,lyrics} or "null"
  python3 - "$1" <<'PYEOF'
import sys, json, re
with open(sys.argv[1]) as f:
    data = json.load(f)
synced = (data.get("syncedLyrics") or "").strip()
plain  = (data.get("plainLyrics")  or "").strip()
if synced:
    timings, lyrics = [], []
    for line in synced.splitlines():
        m = re.match(r'^\[(\d+):(\d+)\.\d+\](.*)', line)
        if m:
            text = m.group(3).strip()
            if text:
                timings.append(int(m.group(1)) * 60 + int(m.group(2)))
                lyrics.append(text)
    if lyrics:
        print(json.dumps({"is_synced": True, "timings": timings, "lyrics": lyrics}))
        sys.exit(0)
if plain:
    lines = [l for l in plain.splitlines() if l.strip()]
    if lines:
        print(json.dumps({"is_synced": False, "timings": None, "lyrics": lines}))
        sys.exit(0)
print("null")
PYEOF
}

fetch_lyrics() {
  # fetch_lyrics <artist> <name> <album> <duration>  → echoes parsed JSON or "null"
  local url="https://lrclib.net/api/get"
  url+="?artist_name=$(urlencode "$1")"
  url+="&track_name=$(urlencode "$2")"
  url+="&album_name=$(urlencode "$3")"
  url+="&duration=$4"
  local code
  code=$(curl -s -o "$TMP_RESP" -w "%{http_code}" "$url" 2>/dev/null || echo "000")
  if [[ "$code" != "200" ]]; then
    echo "HTTP:$code"
    return
  fi
  parse_response "$TMP_RESP"
}

upsert_lyric() {
  # upsert_lyric <song_id> <parsed_json>
  local sid="$1" parsed="$2"
  local entry
  entry=$(python3 -c "
import sys, json
sid = int(sys.argv[1])
d = json.loads(sys.argv[2])
print(json.dumps({
    'song_id': sid,
    'is_synced': d['is_synced'],
    'timings':   d['timings'],
    'lyrics':    d['lyrics'],
}))
" "$sid" "$parsed")
  python3 - "$TMP_LYRICS" "$entry" <<'PYEOF'
import sys, json, os
out  = sys.argv[1]
new  = json.loads(sys.argv[2])
tgt  = new["song_id"]
lines = []
if os.path.exists(out):
    with open(out) as f:
        lines = [l.rstrip("\r\n") for l in f if l.strip()]
found = False
res = []
for line in lines:
    try:
        d = json.loads(line)
        if d.get("song_id") == tgt:
            res.append(json.dumps(new)); found = True; continue
    except Exception:
        pass
    res.append(line)
if not found:
    res.append(json.dumps(new))
with open(out, "w") as f:
    f.write("\n".join(res) + "\n")
PYEOF
}

# Build set of already-fetched song_ids
FETCHED_IDS=$(jq -r '.song_id' "$TMP_LYRICS" 2>/dev/null | sort -u)

FAILURES=()
SUCCESS=0
FAILED=0
SKIPPED_HAVE=0

while IFS= read -r row; do
  [[ -z "$row" ]] && continue
  sid=$(   echo "$row" | jq    '.id')
  name=$(  echo "$row" | jq -r '.name')
  artist=$(echo "$row" | jq -r '.artist')
  album=$( echo "$row" | jq -r '.album')
  dur=$(   echo "$row" | jq    '.duration')

  if echo "$FETCHED_IDS" | grep -qx "$sid"; then
    (( SKIPPED_HAVE++ )) || true
    continue
  fi

  parsed=$(fetch_lyrics "$artist" "$name" "$album" "$dur")
  if [[ "$parsed" == "null" || "$parsed" == HTTP:* ]]; then
    reason="${parsed#HTTP:}"
    [[ "$parsed" == "null" ]] && reason="no lyrics"
    log "  FAIL: $name — $artist ($reason)"
    FAILURES+=("$sid")
    (( FAILED++ )) || true
    continue
  fi

  upsert_lyric "$sid" "$parsed"
  LYRICS_DIRTY=1
  log "  OK:   $name — $artist"
  (( SUCCESS++ )) || true
done < "$TMP_SONGS"

log ""
log "  Bulk pass: $SUCCESS new | $FAILED failed | $SKIPPED_HAVE already had lyrics"

# ─── Phase 7: interactive query fixer ────────────────────────────────────────

if [[ ${#FAILURES[@]} -gt 0 ]]; then
  log ""
  log "==> [6/6] Interactive fixer for ${#FAILURES[@]} failure(s)"
  log "    Edit each field inline (Enter to accept). Empty input keeps the default."
  log ""

  FIXED=0
  for sid in "${FAILURES[@]}"; do
    row=$(jq -c --argjson id "$sid" 'select(.id == $id)' "$TMP_SONGS" | head -1)
    [[ -z "$row" ]] && continue

    name=$(  echo "$row" | jq -r '.name')
    artist=$(echo "$row" | jq -r '.artist')
    album=$( echo "$row" | jq -r '.album')
    dur=$(   echo "$row" | jq    '.duration')

    log ""
    log "── Failed: [$sid] $name — $artist"

    while :; do
      read -e -p "  artist:   " -i "$artist"  artist_in;  [[ -n "$artist_in" ]] && artist="$artist_in"
      read -e -p "  track:    " -i "$name"    name_in;    [[ -n "$name_in"   ]] && name="$name_in"
      read -e -p "  album:    " -i "$album"   album_in;   [[ -n "$album_in"  ]] && album="$album_in"
      read -e -p "  duration: " -i "$dur"     dur_in;     [[ -n "$dur_in"    ]] && dur="$dur_in"

      parsed=$(fetch_lyrics "$artist" "$name" "$album" "$dur")
      if [[ "$parsed" != "null" && "$parsed" != HTTP:* ]]; then
        upsert_lyric "$sid" "$parsed"
        LYRICS_DIRTY=1
        (( FIXED++ )) || true
        log "  ✓ Fixed."
        break
      fi

      reason="${parsed#HTTP:}"
      [[ "$parsed" == "null" ]] && reason="no lyrics"
      log "  Still failing ($reason)."
      read -rp "  [r]etry / [s]kip / [a]bort? " choice
      case "${choice:-s}" in
        r|R) continue ;;
        a|A) log "  Aborting fixer."; break 2 ;;
        *)   log "  Skipped."; break ;;
      esac
    done
  done
  log ""
  log "  Fixer: $FIXED fixed"
fi

# ─── Phase 8: persist lyrics.json (only if changed) ──────────────────────────

log ""
if [[ $LYRICS_DIRTY -eq 1 ]]; then
  cp "$TMP_LYRICS" "$LYRICS_FILE"
  TOTAL_L=$(wc -l < "$LYRICS_FILE" | tr -d ' ')
  log "==> Wrote $LYRICS_FILE  (total entries: $TOTAL_L)"
else
  log "==> lyrics.json unchanged — not writing"
fi

log ""
log "Done."
