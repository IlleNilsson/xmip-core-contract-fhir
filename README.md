# xmip-core-contract-fhir

the FHIR content contract: a well-formed FHIR JSON resource always, Bundles checked through, of a bound resource type and release when a Location names one. Every FHIR release lives here; profiles are the next layer. A technology of [xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

## Toolchain

`rust-toolchain.toml` pins the toolchain for the whole estate. Do not change it
here.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
