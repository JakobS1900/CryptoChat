# CryptoChat DevOps

Automation assets for builds, testing, and releases.

## Subdirectories

- `ci/workflows/` — CI/CD pipeline definitions (GitHub Actions or alternative).
- `packaging/windows/` — Scripts for producing signed MSI installers.
- `packaging/android/` — Gradle tasks and signing configs for AAB/APK artifacts.
- `scripts/` — Reusable automation helpers (PowerShell, Bash, Python).

## Next Steps

- Author initial CI workflow covering lint, test, and workspace builds.
- Document signing key management and release promotion processes.
