# gauchito

A terminal text editor configured in Lua 5.4. The binary is `gau`.

## Install

```sh
cargo install --path cli
```

Needs a stable Rust toolchain. Lua is vendored, nothing else to install.

## Config

`gau` runs `~/.config/gauchito/init.lua` (or `$XDG_CONFIG_HOME/gauchito/init.lua`)
and exits with an error when it is missing. The config gets `gauchito.argv`,
builds its buffers and views, and returns the function the editor runs:

```lua
return function()
    while true do
        local key = gauchito.keys.read()
        if key.ctrl and key.ch == 'q' then gauchito.quit() end
    end
end
```

A config that returns nothing exits right away. The API is typed for
lua-language-server in `script/lua/gauchito.lua`.

## Develop

```sh
cargo run -p cli -- [args]
cargo test --workspace
cargo clippy --workspace --all-targets
```

Crates: `engine` (buffers, splices, selections, motions), `ui` (frame and
renderer), `script` (Lua runtime and tasks), `cli` (terminal loop).
