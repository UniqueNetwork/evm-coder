# Change Log

All notable changes to this project will be documented in this file.

<!-- bureaucrate goes here -->
## [v0.3.0] 2023-02-02

### Added features

- Slightly improve ergonomics 0cdc26eaca5d6102466266e587aa95902c71cf6d

- Custom return value transform b062b56b4bb4e0c83e5c18f114b71bdf9b1e6c36

- Passthru custom attributes to generated call enum 8dd34e67eec554c5e5d5d654c81e47c107f60138

- Separate abi decoding error enum fa588a149d3a660e64bf731b7d29c6246c750dd1

### Other changes

- build: Use workspace package keys 4a5ce68b0a6aeed14316245987051c03e97ee3d3

- style: Enforce stricter formatting 139d86d8531d1e430612acafc20c6f5974c3b939

- test: Use PostInfo for dummy ab341f32d074e97ec23cc8cf36abb3d2f84f8655

- test: Fix is_dynamic failure 7cba253412370a5c315082dd65de292db0289d4f

- style: Fix clippy warnings bcad9995ec79a69a8a701c286e8d39c750e11dd2

- build: Add lockfile 4f7b250dfc1c5df4bcdd71b86cc54de5e4d54b48

- refactor: Remove builtin #[weight] attribute 40517578109812bbff31bb902d990fa4f1c2e4bf

- refactor: Properly process method attributes 9dfe6159aad2f8cd1c4979e84a1b5ad216b70d9f

Instead of requiring dummy attributes to be imported and used,
remove them from the method

- refactor: Drop execution module 8410e224f7a212f0ea60f1443157a145eb8eaa94

## [v0.1.6] - 2023-01-12

### Added
- Support Option<T> type.
### Removed
- Frontier dependency.

## [v0.1.5] - 2022-11-30

### Added
- Derive macro to support structures and enums.

## [v0.1.4] - 2022-11-02

### Added

- Named structures support.

## [v0.1.3] - 2022-08-29

### Fixed

- Parsing simple values.

## [v0.1.2] 2022-08-19

### Added

- Implementation `AbiWrite` for tuples.

### Fixes

- Tuple generation for solidity.

## [v0.1.1] 2022-08-16

### Other changes

- build: Upgrade polkadot to v0.9.27 2c498572636f2b34d53b1c51b7283a761a7dc90a

- build: Upgrade polkadot to v0.9.26 85515e54c4ca1b82a2630034e55dcc804c643bf8

- build: Upgrade polkadot to v0.9.25 cdfb9bdc7b205ff1b5134f034ef9973d769e5e6b
