ARG BASE_ZKVM_IMAGE_TAG=ere-base-zkvm:latest

FROM ${BASE_ZKVM_IMAGE_TAG}

COPY . /ere

WORKDIR /ere

ARG ZKVM
ARG RUSTFLAGS="-Ctarget-cpu=native"

# If current environment is in CI or not.
ARG CI

RUN if [ -n "$CI" ]; then FEATURES=${ZKVM}; else FEATURES=${ZKVM},cuda; fi && \
    RUSTFLAGS=${RUSTFLAGS} cargo build --release --package ere-server --bin ere-server --features $FEATURES && \
    cp /ere/target/release/ere-server /ere/ere-server && \
    cargo clean && \
    rm -rf $CARGO_HOME/registry/src $CARGO_HOME/registry/cache

ENTRYPOINT ["/ere/ere-server"]
