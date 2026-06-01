#!/usr/bin/env bash
# get_lyrics.sh — Fetch timestamped lyrics from LRCLIB for a song list
# Usage: ./get_lyrics.sh <songs.json> [output.json]
#
# Input format (one JSON object per line):
#   {"name":"...","artist":"...","album":"...","duration":123,"id":1, ...}
#
# Output format (one JSON object per line, matches LyricInfo struct):
#   {"song_id":1,"is_synced":true,"timings":[27,31,...],"lyrics":["line",...]}

set -euo pipefail

INPUT="${1:?Usage: $0 <songs.json> [output.json]}"
OUTPUT="${2:-lyrics.json}"
TMPFILE=$(mktemp /tmp/lrclib_XXXXXX.json)
trap 'rm -f "$TMPFILE"' EXIT

# ---------------------------------------------------------------------------
# urlencode <string>  — percent-encode a string for use in a URL query param
# ---------------------------------------------------------------------------
urlencode() {
    python3 -c "import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))" "$1"
}

# ---------------------------------------------------------------------------
# extract_field <json_line> <key>  — pull a single value from a JSON object
# ---------------------------------------------------------------------------
extract_field() {
    python3 -c "
import sys, json
data = json.loads(sys.argv[1])
val = data.get(sys.argv[2])
print('' if val is None else val)
" "$1" "$2"
}

# ---------------------------------------------------------------------------
# parse_response <file>
#   Reads an LRCLIB JSON response; prints a compact {is_synced,timings,lyrics}
#   object, or the bare string 'null' when no usable lyrics are found.
#
#   Timestamps from [mm:ss.cs] are converted to plain u32 seconds.
# ---------------------------------------------------------------------------
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
        # [mm:ss.cs] lyric text
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
# build_output_line <song_id> <parsed_json>
#   Merges song_id into the parsed lyrics object and prints one compact line.
# ---------------------------------------------------------------------------
build_output_line() {
    python3 -c "
import sys, json
song_id = int(sys.argv[1]) if sys.argv[1].isdigit() else None
d = json.loads(sys.argv[2])
out = {
    'song_id':   song_id,
    'is_synced': d['is_synced'],
    'timings':   d['timings'],
    'lyrics':    d['lyrics'],
}
print(json.dumps(out))
" "$1" "$2"
}

# ---------------------------------------------------------------------------
# Main loop
# ---------------------------------------------------------------------------
> "$OUTPUT"
success=0
fail=0

while IFS= read -r line; do
    line="${line%$'\r'}"           # strip Windows CRLF if present
    [[ -z "$line" ]] && continue

    name=$(    extract_field "$line" "name")
    artist=$(  extract_field "$line" "artist")
    album=$(   extract_field "$line" "album")
    duration=$(extract_field "$line" "duration")
    song_id=$( extract_field "$line" "id")

    url="https://lrclib.net/api/get"
    url+="?artist_name=$(urlencode "$artist")"
    url+="&track_name=$(urlencode "$name")"
    url+="&album_name=$(urlencode "$album")"
    url+="&duration=${duration}"

    # Fetch; capture HTTP status separately from body
    http_code=$(curl -s -o "$TMPFILE" -w "%{http_code}" "$url" 2>/dev/null \
                || echo "000")

    if [[ "$http_code" != "200" ]]; then
        printf 'FAIL:    %s  (HTTP %s)\n' "$name" "$http_code"
        fail=$(( fail + 1 ))
        continue
    fi

    parsed=$(parse_response "$TMPFILE")

    if [[ "$parsed" == "null" ]]; then
        printf 'FAIL:    %s  (no lyrics in response)\n' "$name"
        fail=$(( fail + 1 ))
        continue
    fi

    build_output_line "$song_id" "$parsed" >> "$OUTPUT"

    printf 'SUCCESS: %s\n' "$name"
    success=$(( success + 1 ))

done < "$INPUT"

printf '\n%d succeeded, %d failed  →  %s\n' "$success" "$fail" "$OUTPUT"
