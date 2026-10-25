# Jotego Platform Names plugin

A [Pocket plugin](https://github.com/openfpga-library/pocket-plugin) to automatically rename Jotego cores' platform
names step that used to run inside pupdate's Update All

A platform file is only touched while its name is still the raw id. Once renamed, or if you edited
the name yourself, it is left alone, so the plugin is safe to re-run.

## Build

```
rustup target add wasm32-wasip1
scripts/build.sh               # target/wasm32-wasip1/release/plugin.wasm
```

## Test

`cargo test` covers the pure logic (which ids a core owns, when a file needs renaming, which
downloads are accepted).

The sweep can be tested offline against a local copy of the upstream folders, with the built
plugin run headless by `tools/run_plugin.py` (needs `pip install extism`):

```
mkdir -p /tmp/upstream/arcade/Platforms && cp somewhere/jtcps1.json /tmp/upstream/arcade/Platforms/
python3 -m http.server 8766 --bind 127.0.0.1 --directory /tmp/upstream &
python3 tools/run_plugin.py target/wasm32-wasip1/release/plugin.wasm /path/to/sd-card \
    --allow-host 127.0.0.1 --config platform_names_url=http://127.0.0.1:8766/ --host-folder /tmp/plugin-host
```

The `platform_names_url` config key only exists for testing.


## License

MIT (see [LICENSE](LICENSE)).
