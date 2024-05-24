# Living README

These are notes just for me, and intended to be turned into a real README document at a later date.

# How To / What to do

Run a server with `cargo run -- server`

Run one or more clients with `cargo run -- client -c 1`

Open the in-game terminal with *tilde* and type `run_script test.lua` to run a script. Type `help` to view other options.

## Making Maps

### Install [Tiled](https://www.mapeditor.org/)

You can open and edit assets/map_1.tmx to get started.<br>

**IMPORTANT NOTES**
- To see real-time changes in game, you must **disable** Edit > Preferences > Use safe writing of files. (otherwise it creates temporary files instead of overwriting the map)
- When adding a new tileset, just ensure **Embed in Map** is enabled. (otherwise the tileset is not part of the map, so the tileset can't be loaded properly)