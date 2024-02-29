# Setup

## Install Lua

### Download [Lua 5.4.6](https://www.lua.org/download.html)

- Note that the version of Lua is determined by `Cargo.toml`'s `mlua ... features = ["lua54"]` dependency.
- Extract this to a directory above this root project's directory.

### (Windows Only) Install [MSYS2](https://www.msys2.org/)

- Start MSYS2 and install gcc with command `pacman -S base-devel gcc vim cmake`.

### Build Lua

- (Windows Only) Open MSYS2.
- cd into the Lua directory you downloaded.
- Run `make mingw && make install`.