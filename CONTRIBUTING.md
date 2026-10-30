# Contributing

Bug reports, fixes and improvements are welcome. For larger changes, please open an issue first so
that the approach can be agreed before you spend time on it.

See [DEVELOPMENT.md](DEVELOPMENT.md) for setup and [ARCHITECTURE.md](ARCHITECTURE.md) for how the
code fits together.

## What matters here

- **Privacy comes first.** A change that collects new data needs an explicit reason, an opt-in if
  the data is sensitive, and an update to [PRIVACY.md](PRIVACY.md).
- **Never report a guess as fact.** If a platform cannot provide something, say so in its capability
  report. Do not fill the gap with an estimate.
- **Do not bypass platform restrictions.** Wayland's isolation is deliberate.
- **Do not fake statistics.** Everything shown comes from stored records.

## Pull requests

- Keep each pull request to one change.
- Use [conventional commit](https://www.conventionalcommits.org/) messages.
- Add tests for behaviour changes. Session logic belongs in the engine, where it runs in simulated time.
- Run `cargo test -p daytrace-core`, `cargo clippy --workspace --all-targets`, `npm run typecheck` and
  `npm test` before submitting.
- If you change platform behaviour, say what you tested it on and update
  [docs/VERIFICATION.md](docs/VERIFICATION.md).
