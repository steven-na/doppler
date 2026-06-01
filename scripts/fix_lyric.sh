#!/usr/bin/env bash
# fix_lyric.sh — Pick a failed song with fzf and insert its lyrics into lyrics.json
# Usage: ./fix_lyric.sh <songs.json>
#
# lyrics.json is always read/written next to this script, regardless of where
# you call it from.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INPUT="${1:?Usage: $0 <songs.json>}"
OUTPUT="$SCRIPT_DIR/lyrics.json"
TMPFILE=$(mktemp /tmp/lrclib_XXXXXX.json)
trap 'rm -f "$TMPFILE"' EXIT

# ---------------------------------------------------------------------------
# Helpers (mirrors get_lyrics.sh)
# ---------------------------------------------------------------------------
urlencode() {
    python3 -c "import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))" "$1"
}

extract_field() {
    # extract_field <json_string> <key>
    python3 -c "
import sys, json
d = json.loads(sys.argv[1])
v = d.get(sys.argv[2])
print('' if v is None else v)
" "$1" "$2"
}

parse_response() {
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

# ---------------------------------------------------------------------------
# Build fzf list: "<id> TAB <display label>"
# Marks songs that already have an entry in lyrics.json with a check mark.
# ---------------------------------------------------------------------------
fzf_entries=$(python3 - "$INPUT" "$OUTPUT" <<'PYEOF'
import sys, json, os

songs_file  = sys.argv[1]
lyrics_file = sys.argv[2]

# Collect already-fetched song_ids from lyrics.json (if it exists)
fetched = set()
if os.path.exists(lyrics_file):
    with open(lyrics_file) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                d = json.loads(line)
                if d.get("song_id") is not None:
                    fetched.add(d["song_id"])
            except Exception:
                pass

with open(songs_file) as f:
    for raw in f:
        raw = raw.strip()
        if not raw:
            continue
        d = json.loads(raw)
        mark  = "✓" if d["id"] in fetched else " "
        label = f"{mark}  {d['name']}  —  {d['artist']}"
        print(f"{d['id']}\t{label}")
PYEOF
)

# ---------------------------------------------------------------------------
# fzf prompt
# ---------------------------------------------------------------------------
selected=$(printf '%s\n' "$fzf_entries" \
    | fzf --delimiter=$'\t' \
          --with-nth=2 \
          --prompt="Pick song > " \
          --preview-window=hidden \
    ) || { echo "No song selected."; exit 0; }

song_id=$(printf '%s' "$selected" | cut -f1)

# ---------------------------------------------------------------------------
# Look up full song record by id
# ---------------------------------------------------------------------------
song_json=$(python3 -c "
import sys, json
target = int(sys.argv[1])
with open(sys.argv[2]) as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        d = json.loads(line)
        if d.get('id') == target:
            print(json.dumps(d))
            break
" "$song_id" "$INPUT")

name=$(     extract_field "$song_json" "name")
artist=$(   extract_field "$song_json" "artist")
album=$(    extract_field "$song_json" "album")
duration=$( extract_field "$song_json" "duration")

# ---------------------------------------------------------------------------
# Build and display the request URL
# ---------------------------------------------------------------------------
url="https://lrclib.net/api/get"
url+="?artist_name=$(urlencode "$artist")"
url+="&track_name=$(urlencode "$name")"
url+="&album_name=$(urlencode "$album")"
url+="&duration=${duration}"

printf '\n%s\n\n' "$url"

# ---------------------------------------------------------------------------
# Fetch
# ---------------------------------------------------------------------------
http_code=$(curl -s -o "$TMPFILE" -w "%{http_code}" "$url" 2>/dev/null \
            || echo "000")

if [[ "$http_code" != "200" ]]; then
    printf 'Failed to get  %s  (HTTP %s)\n' "$name" "$http_code"
    exit 1
fi

parsed=$(parse_response "$TMPFILE")

if [[ "$parsed" == "null" ]]; then
    printf 'Failed to get  %s  (no lyrics in response)\n' "$name"
    exit 1
fi

# ---------------------------------------------------------------------------
# Build LyricInfo JSON entry
# ---------------------------------------------------------------------------
lyric_entry=$(python3 -c "
import sys, json
sid = int(sys.argv[1])
d   = json.loads(sys.argv[2])
print(json.dumps({
    'song_id':   sid,
    'is_synced': d['is_synced'],
    'timings':   d['timings'],
    'lyrics':    d['lyrics'],
}))
" "$song_id" "$parsed")

# ---------------------------------------------------------------------------
# Upsert into lyrics.json  (replace existing line with same song_id, or append)
# ---------------------------------------------------------------------------
python3 - "$OUTPUT" "$lyric_entry" <<'PYEOF'
import sys, json, os

out_file  = sys.argv[1]
new_entry = json.loads(sys.argv[2])
target_id = new_entry["song_id"]

lines = []
if os.path.exists(out_file):
    with open(out_file) as f:
        lines = [l.rstrip("\r\n") for l in f if l.strip()]

found = False
result = []
for line in lines:
    try:
        d = json.loads(line)
        if d.get("song_id") == target_id:
            result.append(json.dumps(new_entry))
            found = True
            continue
    except Exception:
        pass
    result.append(line)

if not found:
    result.append(json.dumps(new_entry))

with open(out_file, "w") as f:
    f.write("\n".join(result) + "\n")
PYEOF

printf 'Inserting to %s\n' "$OUTPUT"
