# Changelog

## [0.8.1](https://github.com/eth-act/ere/compare/v0.8.0...v0.8.1) (2026-04-23)


### Features

* add `RecordCancellationLayer` layer to record `cancelled` when client drops ([#340](https://github.com/eth-act/ere/issues/340)) ([36f77bb](https://github.com/eth-act/ere/commit/36f77bb2660bdf08d9f650726217f2ab4a7c4371))


### Bug Fixes

* update deps with known issues ([#342](https://github.com/eth-act/ere/issues/342)) ([9237414](https://github.com/eth-act/ere/commit/9237414f1c9a765833ece7713a4d4d18e2025cc0))


### Miscellaneous Chores

* release 0.8.1 ([#343](https://github.com/eth-act/ere/issues/343)) ([3e5250d](https://github.com/eth-act/ere/commit/3e5250dc59205aa8201f9c3f001fc7b60ef327ad))

## [0.8.0](https://github.com/eth-act/ere/compare/v0.7.0...v0.8.0) (2026-04-22)


### Features

* add `--elf-url` support of `ere-server` ([#333](https://github.com/eth-act/ere/issues/333)) ([a225cde](https://github.com/eth-act/ere/commit/a225cded97173b2a17a56a5b560820712fbaed00))
* add metrics support in `ere-server` ([#335](https://github.com/eth-act/ere/issues/335)) ([7871f1a](https://github.com/eth-act/ere/commit/7871f1a0436db44734e0aeadaa31a8cafbc9032b))
* impl Encode and Decode for Vec&lt;u8&gt; and [u8; N] ([#339](https://github.com/eth-act/ere/issues/339)) ([8cf4e16](https://github.com/eth-act/ere/commit/8cf4e16b01fcc5f8a03e2dd06371efb3eae19f54))
* remove `Compile::Program` ([#331](https://github.com/eth-act/ere/issues/331)) ([008abe1](https://github.com/eth-act/ere/commit/008abe141ec05bed8704564ab57c526c095a832e))
* split zkVM trait into prover and verifier ([#332](https://github.com/eth-act/ere/issues/332)) ([e12e7ba](https://github.com/eth-act/ere/commit/e12e7baae009ec91331791c8400a82b39fdac0f3))
* support multi cuda archs for zisk ([#337](https://github.com/eth-act/ere/issues/337)) ([8401f02](https://github.com/eth-act/ere/commit/8401f025325660ad6d30f0037c54860d897fc7eb))
* update `risc0` to `v3.0.5` and use its latset rust release ([#329](https://github.com/eth-act/ere/issues/329)) ([be21c34](https://github.com/eth-act/ere/commit/be21c3407605aa86e6345a9f0be92dba75171a66))
* use upstream `ziskos` ([#334](https://github.com/eth-act/ere/issues/334)) ([2480381](https://github.com/eth-act/ere/commit/24803815d26aeb276d56cba5313ddc9ecfd69815))


### Bug Fixes

* enable `rustls-tls` for `ere-server` ([#338](https://github.com/eth-act/ere/issues/338)) ([ae2a8b3](https://github.com/eth-act/ere/commit/ae2a8b371525fbee786f2a5cc63e401e2d1c1db9))

## [0.7.0](https://github.com/eth-act/ere/compare/v0.6.1...v0.7.0) (2026-04-07)


### Features

* add `DockerizedzkVMConfig` to allow specify operation timeouts ([#324](https://github.com/eth-act/ere/issues/324)) ([3914a12](https://github.com/eth-act/ere/commit/3914a12f18b5f27807114048ae4829f3e999806b))
* add telementry support for ere-server ([#328](https://github.com/eth-act/ere/issues/328)) ([7a6a471](https://github.com/eth-act/ere/commit/7a6a4716cf639055f9778551877f42177f0c44c4))

## [0.6.1](https://github.com/eth-act/ere/compare/v0.6.0...v0.6.1) (2026-03-28)


### Bug Fixes

* the docker container retry logics ([#318](https://github.com/eth-act/ere/issues/318)) ([e0a0553](https://github.com/eth-act/ere/commit/e0a0553d6dbd9705756f0502de914109d1682847))
* update zisk patch rev to `75957ca` with fix ([#322](https://github.com/eth-act/ere/issues/322)) ([17c78e0](https://github.com/eth-act/ere/commit/17c78e0cf276d881f171112f9e58eba8aa157639))
