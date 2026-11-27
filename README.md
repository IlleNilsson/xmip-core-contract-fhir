# xmip-core-contract-fhir

the FHIR content contract: a well-formed FHIR JSON resource always, Bundles checked through, of a bound resource type and release when a Location names one. Every FHIR release lives here; profiles are the next layer. A technology of [xmip-core-contract](https://github.com/IlleNilsson/xmip-core-contract).

Each departure names where it is as a JSON Pointer (`/entry/1/resource/id`),
spelled only when one is raised.

## The declaration

What a Location naming this contract is configured with is two `const`s in
its `declaration` module, `src/declaration.rs`, beside the code, which its
`ContractFactory` reads (ADR-0064, amendment 2026-11-26):
`declaration::SETTINGS`, `reference`, the resource type a resource is, and
`NAME`, `fhir`, the name a Validate step's `format` names it by. Built with
its `declaration` feature alone (`--no-default-features --features
declaration`), the crate is those and nothing more: its code and every crate
the code is built on are behind the default feature. So the library the
surfaces load can carry it without the code, for the language server and the
desktop to suggest and check a Location's contract settings though no node
in their process loaded the technology, once the gathering crate of open
problem 48 links it. `test/Declaration.Test.ps1` in the estate holds that
the declaration builds nothing the contract capability does not.

## Verification

The included workflow is manual-only and calls the versioned shared workflow at
`IlleNilsson/.github@v1`.
