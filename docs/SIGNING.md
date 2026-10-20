# Code signing: what to buy and where to put it

Unsigned apps trigger Windows SmartScreen ("Windows protected your PC") and macOS Gatekeeper
("can't be opened because Apple cannot check it"). Signing removes those warnings. The release
workflow turns signing on by itself once these repository secrets exist
(GitHub → sriptcollector/toolbay-flow → Settings → Secrets and variables → Actions).

## Windows: Azure Trusted Signing (cheapest that works with CI)

- **Buy:** an Azure subscription with **Trusted Signing**, Basic plan, about $9.99/month.
- **Identity validation:** Microsoft verifies the person or company. For an individual it's done in the
  Azure portal with a government ID. A company needs to have existed for 3+ years for "public trust",
  otherwise sign as an individual.
- **Create:** a Trusted Signing account and a certificate profile (Public Trust), and an app
  registration (service principal) with the role "Trusted Signing Certificate Profile Signer".
- **Secrets to add:** `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`,
  `TRUSTED_SIGNING_ACCOUNT`, `TRUSTED_SIGNING_PROFILE`.
- SmartScreen reputation builds up over the first few hundred downloads even when signed.
  An EV certificate (about $250-400/year from SSL.com, Sectigo or DigiCert, on a hardware token)
  skips that ramp but is harder to use from CI.

## macOS: Apple Developer Program

- **Buy:** Apple Developer Program membership, $99/year (developer.apple.com/programs).
- **Create:** a "Developer ID Application" certificate in Xcode or the developer portal, export it as
  a .p12 with a password, and base64 it (`base64 -i cert.p12 | pbcopy`).
- **Create:** an app-specific password at appleid.apple.com for notarization.
- **Secrets to add:** `APPLE_CERTIFICATE` (the base64 .p12), `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY` (e.g. `Developer ID Application: Your Name (TEAMID)`), `APPLE_ID`,
  `APPLE_PASSWORD` (the app-specific password), `APPLE_TEAM_ID`.

## Update signing (already set up)

`TAURI_SIGNING_PRIVATE_KEY` signs the update packages that installed copies download. It's a
free key generated locally (kept at `~/.tauri/toolbay-flow-updater.key` on the build PC, never in
the repo). Its public half is in `src-tauri/tauri.conf.json`. Back it up: losing it means existing
installs can't auto-update.
