# Changelog

## [0.16.3](https://github.com/eth-act/ere/compare/v0.16.2...v0.16.3) (2026-08-25)


### Bug Fixes

* openvm zkvm accelerator ([#413](https://github.com/eth-act/ere/issues/413)) ([a956a62](https://github.com/eth-act/ere/commit/a956a6239b9ee455345445aad8900334db43b2dc))

## [0.16.2](https://github.com/eth-act/ere/compare/v0.16.1...v0.16.2) (2026-08-21)


### Bug Fixes

* disable avx512 for zisk prover ([#410](https://github.com/eth-act/ere/issues/410)) ([2d1529a](https://github.com/eth-act/ere/commit/2d1529a965bae28ead531333bfae8c45300a3a13))
* use arg for MAKEFLAGS for overriding ([#412](https://github.com/eth-act/ere/issues/412)) ([3f854dd](https://github.com/eth-act/ere/commit/3f854dd728fa9f9af7b299293338b3f79e104492))

## [0.16.1](https://github.com/eth-act/ere/compare/v0.16.0...v0.16.1) (2026-08-20)


### Bug Fixes

* update vk to new zisk pk/vk ([#408](https://github.com/eth-act/ere/issues/408)) ([bf4ebfb](https://github.com/eth-act/ere/commit/bf4ebfbf5c793d715369783121dc20f4c990213a))

## [0.16.0](https://github.com/eth-act/ere/compare/v0.15.0...v0.16.0) (2026-08-19)


### Features

* add cancel timeout on the prove helper func ([#407](https://github.com/eth-act/ere/issues/407)) ([8295d94](https://github.com/eth-act/ere/commit/8295d94b4598ef14e6c35fff3dd4935f60f2f7ff))
* udpate sp1 to v6.4.0 ([#406](https://github.com/eth-act/ere/issues/406)) ([0e27a37](https://github.com/eth-act/ere/commit/0e27a3759d1c31301890a5645b256cfaccd2fdd0))
* update zisk to v1.1.0-alpha ([#404](https://github.com/eth-act/ere/issues/404)) ([4a5a1ab](https://github.com/eth-act/ere/commit/4a5a1ab822f496d5cefcd870adb74f9a91dbd6c9))

## [0.15.0](https://github.com/eth-act/ere/compare/v0.14.0...v0.15.0) (2026-08-06)


### Features

* add --ignore-rust-version support ([#396](https://github.com/eth-act/ere/issues/396)) ([7e8e2ba](https://github.com/eth-act/ere/commit/7e8e2ba1001f7776547f0576e4eb49bd0155e0d2))
* add health_timeout to DockerizedzkVMConfig ([#398](https://github.com/eth-act/ere/issues/398)) ([c8d621d](https://github.com/eth-act/ere/commit/c8d621df229526fbab3c8012656c38213e079b50))
* impl zkvm_accelerators.h by openvm guest libs ([#400](https://github.com/eth-act/ere/issues/400)) ([153b092](https://github.com/eth-act/ere/commit/153b092064a11e4c9c031a89940cbb232c98d59d))
* improve openvm zkvm_accelerator impl ([#401](https://github.com/eth-act/ere/issues/401)) ([f50e1d9](https://github.com/eth-act/ere/commit/f50e1d9d7e9cdfef5511dd71018c907d217b7a48))
* update openvm to v2.1.0-preview ([#397](https://github.com/eth-act/ere/issues/397)) ([6bc7dfc](https://github.com/eth-act/ere/commit/6bc7dfc4a02e7c82ca9667a9bd6b7c4723900a6b))


### Bug Fixes

* detect openvm version by crate openvm ([#399](https://github.com/eth-act/ere/issues/399)) ([a146b0c](https://github.com/eth-act/ere/commit/a146b0c65e80c4ac32d0520adfab41ff226b08a1))
* sync Cargo.lock ([#394](https://github.com/eth-act/ere/issues/394)) ([5b038b2](https://github.com/eth-act/ere/commit/5b038b2725fcaff8cff2fef44f20d6457ebe2385))

## [0.14.0](https://github.com/eth-act/ere/compare/v0.13.0...v0.14.0) (2026-07-30)


### Features

* add verification key generation document ([#393](https://github.com/eth-act/ere/issues/393)) ([209b7f3](https://github.com/eth-act/ere/commit/209b7f34c3d694e80b4f98223fa0d20907cde014))
* bump openvm to v2.0 official release ([#389](https://github.com/eth-act/ere/issues/389)) ([1e671da](https://github.com/eth-act/ere/commit/1e671da94655cbcd058d9b43d6819792e77a04d9))
* propagate rustflags ([#391](https://github.com/eth-act/ere/issues/391)) ([58ca85b](https://github.com/eth-act/ere/commit/58ca85beaee2fa8acd31dbf33b90bb765aac9010))


### Bug Fixes

* rebuild zisk image ([#392](https://github.com/eth-act/ere/issues/392)) ([a25f1ae](https://github.com/eth-act/ere/commit/a25f1aed9664c3b63e73ef05360090a4c41da31b))

## [0.13.0](https://github.com/eth-act/ere/compare/v0.12.2...v0.13.0) (2026-07-02)


### Features

* import sp1-libzkevm into ere-platform-sp1 to include zkvm-standards impl ([#388](https://github.com/eth-act/ere/issues/388)) ([968d72e](https://github.com/eth-act/ere/commit/968d72e1ca55e8744ff3517460f0bbd3ee88bd4b))
* openvm v2.0 ([#387](https://github.com/eth-act/ere/issues/387)) ([cb14872](https://github.com/eth-act/ere/commit/cb148727f0a0606f16e91106b7fba9b7a462ef04))
* update sp1 v6.3.0 ([#383](https://github.com/eth-act/ere/issues/383)) ([aea6655](https://github.com/eth-act/ere/commit/aea6655c3fa605e946c61457193eacc3a0ea774e))
* zisk v1.0.0-alpha ([#385](https://github.com/eth-act/ere/issues/385)) ([2505e96](https://github.com/eth-act/ere/commit/2505e96eb9821b7aa561b56eeeddc145baa138fd))

## [0.12.2](https://github.com/eth-act/ere/compare/v0.12.1...v0.12.2) (2026-06-16)


### Bug Fixes

* use patch for ziskos with fixes of precompile impls ([#381](https://github.com/eth-act/ere/issues/381)) ([c5cf11e](https://github.com/eth-act/ere/commit/c5cf11e09efad10dab0630ff748410d541b24293))

## [0.12.1](https://github.com/eth-act/ere/compare/v0.12.0...v0.12.1) (2026-06-08)


### Bug Fixes

* expose setup of zisk cluster client ([#378](https://github.com/eth-act/ere/issues/378)) ([79962c0](https://github.com/eth-act/ere/commit/79962c00d9fe615393f2e0b39dc9b27e47a6aac7))

## [0.12.0](https://github.com/eth-act/ere/compare/v0.11.0...v0.12.0) (2026-06-08)


### Features

* add passes=lower-atomic to zisk customized target compiler ([#371](https://github.com/eth-act/ere/issues/371)) ([224b058](https://github.com/eth-act/ere/commit/224b058a77263670d9c9286a62ab90a4c582108b))
* improve zisk cluster client ([#373](https://github.com/eth-act/ere/issues/373)) ([5e9a70e](https://github.com/eth-act/ere/commit/5e9a70e1436829046d1f04696f53cc9bf6aa1b14))
* retry to survive coordinator down ([#375](https://github.com/eth-act/ere/issues/375)) ([eba2bc3](https://github.com/eth-act/ere/commit/eba2bc32526d4eef1a4d796eae6dc08f649e8963))
* verifier binding c and go ([#377](https://github.com/eth-act/ere/issues/377)) ([658473b](https://github.com/eth-act/ere/commit/658473ba58aeec59022fd33fec1163d62e36cfb5))


### Bug Fixes

* docker build of sp1 and risc0 ([#376](https://github.com/eth-act/ere/issues/376)) ([8230df9](https://github.com/eth-act/ere/commit/8230df91c155269299e4188dec933cfbc283c653))


### Performance Improvements

* add optimized llvm parameters for the zisk target ([#374](https://github.com/eth-act/ere/issues/374)) ([e2023a5](https://github.com/eth-act/ere/commit/e2023a54d0f3a5eac601a92da92862cf96554d95))

## [0.11.0](https://github.com/eth-act/ere/compare/v0.10.0...v0.11.0) (2026-05-21)


### Features

* refactor platform io ([#369](https://github.com/eth-act/ere/issues/369)) ([4b113ca](https://github.com/eth-act/ere/commit/4b113ca46b9d02400c2da8fbc59db07df935ac7c))

## [0.10.0](https://github.com/eth-act/ere/compare/v0.9.1...v0.10.0) (2026-05-20)


### Features

* add verifier benchmark ([#366](https://github.com/eth-act/ere/issues/366)) ([cc9fb07](https://github.com/eth-act/ere/commit/cc9fb07e843420c302b6d227593df4c1625ba6cb))
* extend cuda archs support of published images ([#364](https://github.com/eth-act/ere/issues/364)) ([e360a15](https://github.com/eth-act/ere/commit/e360a1538d55ff9665e65c390c68a96a9458e36f))
* update zisk to v0.18.0 ([#367](https://github.com/eth-act/ere/issues/367)) ([5ca9e05](https://github.com/eth-act/ere/commit/5ca9e05fc479a7ae49d667c04a772b69ba7cff08))
* zisk program vk without prover ([#368](https://github.com/eth-act/ere/issues/368)) ([d9c94a5](https://github.com/eth-act/ere/commit/d9c94a5b1c9161ae0e33f53a977ef3b977991e0a))

## [0.9.1](https://github.com/eth-act/ere/compare/v0.9.0...v0.9.1) (2026-05-12)


### Bug Fixes

* add `shm-size` config for SP1 ([#360](https://github.com/eth-act/ere/issues/360)) ([4bdabee](https://github.com/eth-act/ere/commit/4bdabee0fbbe28717967335b91274ff50c2e838a))

## [0.9.0](https://github.com/eth-act/ere/compare/v0.8.1...v0.9.0) (2026-05-12)


### Features

* add a binary semaphore to ere-server prove endpoint ([#348](https://github.com/eth-act/ere/issues/348)) ([580a70a](https://github.com/eth-act/ere/commit/580a70ae9cb873c21f91db9e282f43a57434de6a))
* add crate `ere-verifier` and test fixture for verifier crates ([#359](https://github.com/eth-act/ere/issues/359)) ([cb84829](https://github.com/eth-act/ere/commit/cb8482925d2fa05fb3ed8c11024834cd0f48b6df))
* finer modules ([#346](https://github.com/eth-act/ere/issues/346)) ([44e91ed](https://github.com/eth-act/ere/commit/44e91ed7c0bd5398d55eb4ce43594ac0f161dd2b))
* fix airbender verifier ([#357](https://github.com/eth-act/ere/issues/357)) ([86cca13](https://github.com/eth-act/ere/commit/86cca13141a4c5128bd5c2bd50ade99868951cb7))
* **provers:** Add optional cycle tracking to proving report ([#358](https://github.com/eth-act/ere/issues/358)) ([918985c](https://github.com/eth-act/ere/commit/918985c07cb66c151f2eb4f28d9527e730c7267b))
* support `--feature` args in `Compiler::compile` ([#355](https://github.com/eth-act/ere/issues/355)) ([073b419](https://github.com/eth-act/ere/commit/073b41993f2262bbaec7ccc05cd2c9d93ebc2f3d))
* update `detect_sdk_version` ([#353](https://github.com/eth-act/ere/issues/353)) ([ef7a4cc](https://github.com/eth-act/ere/commit/ef7a4ccd94cfcd700c3a3058658e0814915cb09e))
* update `sp1` to `v6.1.0` ([#349](https://github.com/eth-act/ere/issues/349)) ([02659ca](https://github.com/eth-act/ere/commit/02659ca3701f9d013ac89fded4aa250c29256fb6))
* update airbender to use sdk ([#347](https://github.com/eth-act/ere/issues/347)) ([ad57b1d](https://github.com/eth-act/ere/commit/ad57b1d74db3c56a7e1413ab29300f38d9a8c864))
* vendor verifier from `openvm-sdk` to avoid pulling in `openvm` as dep ([#356](https://github.com/eth-act/ere/issues/356)) ([2118a31](https://github.com/eth-act/ere/commit/2118a315599a2460c483d8ac119616c4b6700169))
* zisk cluster verify ([#344](https://github.com/eth-act/ere/issues/344)) ([21b40df](https://github.com/eth-act/ere/commit/21b40dfeea610c430ede65be055041ea4117694b))


### Bug Fixes

* zisk sdk install script to pin ziskup ([#354](https://github.com/eth-act/ere/issues/354)) ([e63f6e8](https://github.com/eth-act/ere/commit/e63f6e8713bd429d80fcd188f893e98a17a3574e))

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
