# gauchito

A Lua API to build your own terminal editor. The binary is `gau`.

## Install

```sh
cargo install gauchito-cli
```

Or grab a binary from the GitHub releases. Lua is vendored, nothing else to
install.

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
lua-language-server in `gauchito-script/lua/gauchito.lua`.

## Develop

```sh
cargo build --workspace                      # build
cargo run -p gauchito-cli -- [args]          # run gau
cargo test --workspace                       # test
cargo clean                                  # remove build artifacts
cargo fmt -- --check                         # check formatting
cargo clippy --workspace --all-targets       # lint
cargo clippy --workspace --fix --allow-dirty && cargo fmt   # fix
```

Crates: `gauchito-core` (buffers, splices, selections, motions), `gauchito-ui`
(frame and renderer), `gauchito-script` (Lua runtime and tasks), `gauchito-cli`
(terminal loop, the `gau` binary).
