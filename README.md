# Doppler - A simple Rust TUI music player

## A terminal music player with Spotify import and synced lyrics

This project is a simple and easy to use terminal music player. Once you have imported the songs from any Spotify playlist, you can listen offline even with synced lyrics.

- Import songs from Spotify. `build_library.sh` fetches mp3s, plyalist metadata, and synced song lyrics from LRCLib.net
- Live updating TUI written in Rust with Ratatui
- Create custom playlists to play and add/remove songs
- View lyrics with live syncing

## How to use Doppler

### What you'll need:
- Rust language
- Spotify-dl (Make sure to check the [dependencies](#dependencies-and-required-programs)) section

### Start fresh:

1. Clone the repo:
    - `git clone https://github.com/steven-na/doppler.git`
    - `cd doppler`
2. Import your library/playlists
    - `bash ./scripts/build_library.sh` and follow the directions. You will need to create a Spotify developer app to download the playlist metadata. (https://developer.spotify.com/)
    - If you'd like, you can relocate or symlink the `songs.json`, `lyrics.json`, and `mp3/` files to another location
3. Once you have finished importing songs, run `cargo run -- ./scripts` in the Doppler directory

### Controls
- `Ctrl-c` exit (when not typing)
- `Up/Down Arrow` navigate lists
- `Tab` switch between songs/playlists panes
- `Space` play song/playlist or add song to playlist (in playlist edit mode)
- `V/v` increase/decrease volume
- `q` enqueue song
- `k` skip song
- `p` pause
- `Shft-Q` open queue pane
- `Shft-L` open lyrics pane
- `f` open playlist edit pane

There are other controls that are explained within the app when necessary

### If you already have a library in the required format, 
simply clone this repo and run `cargo run -- './library directory'`

The binary will look for a `./data` directory from the run location by default

## Find a bug?

If you find a bug or would like to help improve the project, submit an issue on this repo. If you submit a PR, make sure to reference the issue you created.

## Known issues/Planned changes

This project is still under development, so issues may arise

- More interactions with UI are being implemented (removing playlists, etc.)
- Better UI updates (Sync annoy loop with song playing)

## Dependencies and Required programs

- Rust (https://rustup.rs/)
- Spotify-dl (https://github.com/dcordonu/spotify-dl)
- python3 (https://www.python.org/)
- fzf (`sudo apt install fzf`)
- curl (`sudo apt install curl`)
- jq (`sudo apt install jq`)
