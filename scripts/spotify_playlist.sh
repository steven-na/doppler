#!/usr/bin/env bash
# spotify_playlist.sh — Fetch all tracks from a Spotify playlist and save to JSON
# Usage: ./spotify_playlist.sh <playlist_url_or_id>
#
# Requires: curl, jq
# Set your Spotify app credentials as env vars before running:
#   export SPOTIFY_CLIENT_ID="your_client_id"
#   export SPOTIFY_CLIENT_SECRET="your_client_secret"
#
# Get credentials at: https://developer.spotify.com/dashboard

set -euo pipefail

# ── Helpers ──────────────────────────────────────────────────────────────────

die()  { echo "ERROR: $*" >&2; exit 1; }
log()  { echo "$*" >&2; }
need() { command -v "$1" &>/dev/null || die "'$1' is required but not installed."; }

need curl
need jq

# ── Credentials ───────────────────────────────────────────────────────────────

CLIENT_ID="${SPOTIFY_CLIENT_ID:-}"
CLIENT_SECRET="${SPOTIFY_CLIENT_SECRET:-}"

if [[ -z "$CLIENT_ID" || -z "$CLIENT_SECRET" ]]; then
  echo "Spotify credentials not found in environment."
  read -rp "Enter your Spotify Client ID:     " CLIENT_ID
  read -rp "Enter your Spotify Client Secret: " CLIENT_SECRET
fi

[[ -z "$CLIENT_ID" || -z "$CLIENT_SECRET" ]] && die "Client ID and Secret are required."

# ── Playlist ID ───────────────────────────────────────────────────────────────

INPUT="${1:-}"
if [[ -z "$INPUT" ]]; then
  read -rp "Enter Spotify playlist URL or ID: " INPUT
fi

# Extract ID from a full URL (e.g. https://open.spotify.com/playlist/37i9dQZF1DX...)
PLAYLIST_ID=$(echo "$INPUT" | sed -E 's|.*playlist/([A-Za-z0-9]+).*|\1|')
[[ -z "$PLAYLIST_ID" ]] && die "Could not parse a playlist ID from: $INPUT"

log "Playlist ID: $PLAYLIST_ID"

# ── Auth: Client Credentials flow ────────────────────────────────────────────

log "Authenticating with Spotify..."
AUTH_RESPONSE=$(curl -s -X POST "https://accounts.spotify.com/api/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  --data-urlencode "grant_type=client_credentials" \
  --data-urlencode "client_id=$CLIENT_ID" \
  --data-urlencode "client_secret=$CLIENT_SECRET")

ACCESS_TOKEN=$(echo "$AUTH_RESPONSE" | jq -r '.access_token // empty')
[[ -z "$ACCESS_TOKEN" ]] && die "Failed to get access token. Check your credentials.\n$AUTH_RESPONSE"

log "Authenticated."

# ── Fetch all tracks (paginated) ──────────────────────────────────────────────

TRACKS_JSON="[]"
OFFSET=0
LIMIT=100
TOTAL=-1
PLAYLIST_NAME=""

log "Fetching tracks..."

while true; do
  RESPONSE=$(curl -s -G "https://api.spotify.com/v1/playlists/${PLAYLIST_ID}/tracks" \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    --data-urlencode "offset=$OFFSET" \
    --data-urlencode "limit=$LIMIT" \
    --data-urlencode "fields=total,items(track(name,duration_ms,artists(name),album(name,release_date)))")

  # Check for API errors
  ERROR=$(echo "$RESPONSE" | jq -r '.error.message // empty')
  [[ -n "$ERROR" ]] && die "Spotify API error: $ERROR"

  # Grab total once
  if [[ $TOTAL -eq -1 ]]; then
    TOTAL=$(echo "$RESPONSE" | jq '.total')

    # Fetch playlist name separately (not in tracks endpoint)
    META=$(curl -s "https://api.spotify.com/v1/playlists/${PLAYLIST_ID}?fields=name" \
      -H "Authorization: Bearer $ACCESS_TOKEN")
    PLAYLIST_NAME=$(echo "$META" | jq -r '.name')
    log "Playlist: $PLAYLIST_NAME ($TOTAL tracks)"
  fi

  # Parse this page's tracks into clean objects
  PAGE_TRACKS=$(echo "$RESPONSE" | jq '[
    .items[]
    | select(.track != null)
    | .track
    | {
        name:             .name,
        artist:           ( [.artists[].name] | join(", ") ),
        album:            .album.name,
        release_date:     .album.release_date,
        duration_seconds: ( .duration_ms / 1000 | floor )
      }
  ]')

  # Merge into running list
  TRACKS_JSON=$(jq -n --argjson acc "$TRACKS_JSON" --argjson page "$PAGE_TRACKS" '$acc + $page')

  FETCHED=$(( OFFSET + LIMIT ))
  OFFSET=$FETCHED

  log "  Fetched $( [[ $FETCHED -lt $TOTAL ]] && echo $FETCHED || echo $TOTAL ) / $TOTAL"

  [[ $FETCHED -ge $TOTAL ]] && break
done

# ── Output JSON to stdout ─────────────────────────────────────────────────────

jq -n \
  --arg name "$PLAYLIST_NAME" \
  --arg id "$PLAYLIST_ID" \
  --argjson tracks "$TRACKS_JSON" \
  '{
    playlist_name: $name,
    playlist_id:   $id,
    track_count:   ($tracks | length),
    tracks:        $tracks
  }'

log "Done. Tracks exported: $(echo "$TRACKS_JSON" | jq 'length')"
