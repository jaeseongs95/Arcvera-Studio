# Source-bound Native development executable

[한국어](native-fixture-artifact.ko.md)

The earlier `3830a87e` executable's manifest covered only selected Native files. It does not establish full-checkout/frontend/commit binding. Additional Native review and Core1.1 integration will use the new CI executable with its manifests.

After targeted unit tests and non-test `cargo check`, CI builds a separate-identifier app from the checkout with `--debug --no-bundle --ci`. It does not install, start services or run the app. Verify the identifier recorded in `fixture.json` against the actual app. It differs from the production identifier; do not run the same fixture concurrently.

Acceptance cases:

- AF01: Reject tracked changes from HEAD before/after the build. Record the40-character commit and hashes of every tracked file.
- AF02: Record executable hash, identifier, actual argv and generated frontend/notice/config hashes. Derive sidecars from the resource map; reject paths outside checkout/app.
- AF03: Failed build or missing executable produces no success manifest; existing app evidence is not overwritten.
- AF04: After extraction, verify executable/manifest hashes, then use an own HOME/project/profile and unique CDP port and check the actual identifier. Running it belongs to a separate review/integration step.

`desktop-native-adapter-validation` contains `fixture.json`, `source-manifest.json`, `generated-manifest.json`, `app/` and unit logs. The new CI build, AF01–03 execution and AF04 Native execution are **NOT_RUN** at authoring. Syntax checks/hashes do not establish real Core1.1 integration, final packaging or release approval.
