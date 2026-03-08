id := "st.lynx.plugins.darktable.sdPlugin"

build:
    cargo build --release

check:
    ./release-check.sh

collect: build
    rm -rf build
    mkdir -p build/{{id}}
    cp -r assets build/{{id}}
    cp manifest.json pi.html README.md build/{{id}}
    cp target/release/opendeck-darktable build/{{id}}/opendeck-darktable

[working-directory: "build"]
package: collect
    zip -r darktable.plugin.zip {{id}}/
