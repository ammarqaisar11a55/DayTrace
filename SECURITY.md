# Security

## Reporting a vulnerability

Report vulnerabilities privately through
[GitHub security advisories](https://github.com/ammarqaisar11a55/DayTrace/security/advisories/new).
Do not open a public issue. Please include the version, your operating system and steps to reproduce.

## Supported versions

Only the latest release receives security fixes.

## How DayTrace protects you

- **Signed updates.** Every update is verified with a minisign signature against a public key
  compiled into the application, before installation. An update that fails is never installed.
- **No downgrades.** Only versions newer than the installed one are accepted, and the stable
  channel never accepts prereleases.
- **HTTPS only.** Update checks and downloads use HTTPS.
- **Least privilege.** Tracking needs no administrator or root access. The webview has only the
  core Tauri permissions and a strict content security policy, and it has no direct access to the
  filesystem or database.
- **Prepared SQL.** All queries use bound parameters.
- **Safe exports.** CSV exports neutralize values a spreadsheet would run as formulas, because
  window titles are text that websites and documents control.

## Signing keys

The updater's private key is held in GitHub Actions secrets and must never be committed. If it is
lost or exposed, a new key must be issued and shipped in a release signed by the old one, because
installed copies only trust the key they were built with.

Windows installers are not yet code-signed, so Windows SmartScreen may warn about them. This is
separate from update signing.
