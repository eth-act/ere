ARG BASE_ZKVM_IMAGE_TAG=ere-base-zkvm:latest

FROM ${BASE_ZKVM_IMAGE_TAG}

COPY . /ere

WORKDIR /ere

ARG ZKVM
ARG RUSTFLAGS="-Ctarget-cpu=native"

# If current environment is in CI or not.
ARG CI

RUN RUSTFLAGS=${RUSTFLAGS} cargo build --release --package ere-cli --bin ere-cli --features cli,${ZKVM} ${CI:+--no-default-features} && \
    cp /ere/target/release/ere-cli /ere/ere-cli && \
    cargo clean && \
    rm -rf $CARGO_HOME/registry/src $CARGO_HOME/registry/cache

ENTRYPOINT ["/ere/ere-cli"]
