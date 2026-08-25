# Reference build recipe for running looprs inside a minibox container.
#
# `mbx build` is not available in this build of minibox (no `build`
# subcommand — the pipeline is still experimental per minibox's own docs),
# so this Dockerfile is not consumed directly by `mbx`. It documents the
# exact steps the run-minibox skill's driver replays manually via
# `mbx run ... -v <repo>:/src rust:1-alpine -- sh -c '<these same commands>'`.
# If/when `mbx build` lands, `mbx build -f Dockerfile .` should work as-is.
#
# Verified working recipe as of 2026-08-23 (macOS, smolvm adapter).
FROM rust:1-alpine

# musl-dev/pkgconfig/openssl-dev: base Alpine build toolchain for a Rust
# crate with C deps (reqwest -> native-tls -> openssl-sys). openssl-libs-static
# gives openssl-sys a static musl-linkable libssl/libcrypto so the resulting
# looprs binary has no runtime shared-lib dependency.
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

# NOTE: minibox does not propagate the image's ENV PATH into the container
# (confirmed: `echo $PATH` inside a running rust:1-alpine container omits
# /usr/local/cargo/bin even though the image's own Dockerfile sets it) —
# any command invoking cargo/rustc must reference /usr/local/cargo/bin
# explicitly rather than relying on `cargo` resolving via PATH.
ENV PATH="/usr/local/cargo/bin:${PATH}"

WORKDIR /src
COPY . /src

RUN cargo build --release -p looprs-cli

ENTRYPOINT ["/src/target/release/looprs"]
