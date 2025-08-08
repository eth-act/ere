ARG BASE_ZKVM_IMAGE_TAG=ere-base-zkvm:latest

FROM ${BASE_ZKVM_IMAGE_TAG}

COPY . /ere

WORKDIR /ere

ARG ZKVM

RUN cargo build --release --package ere-cli --bin ere-cli --features cli,${ZKVM} && \
    cp /ere/target/release/ere-cli /ere/ere-cli && \
    cargo clean && \
    rm -rf $CARGO_HOME/registry/src $CARGO_HOME/registry/cache

ENTRYPOINT ["/ere/ere-cli"]
