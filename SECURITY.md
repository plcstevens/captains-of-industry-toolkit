# Security policy

## Supported versions

This is a pre-1.0 project maintained by one person. Only the
**latest release** is supported: security fixes land in new
releases and are not backported to older versions. If you find
a problem, please check whether it is still present in the
current release — and say which version you were running when
you report it.

## Reporting a vulnerability

Please report vulnerabilities privately, through GitHub's
private vulnerability reporting:

1. Open this repository's **Security** tab.
2. Choose **Report a vulnerability**.
3. Describe what you found: the affected component and version,
   what it lets someone do, and the steps to reproduce it if
   you have them.

Please do not open a public issue for a security problem, and
do not disclose it publicly before the maintainer has had a
reasonable chance to look at it and ship a fix — see
"Disclosure" below.

## Scope

In scope — the software in this repository:

- **Superintendent** — the companion app (`coi/`).
- **CoiBridge** — the read-only bridge mod (`bridge/`).
- **The QoS mod suite** — QoSCore and the feature mods
  (`qos/`).
- **QoSBuilder** — the placement and designation mod
  (`qos/QoSBuilder/`).

Out of scope:

- **The game itself.** Vulnerabilities in Captain of Industry
  or its built-in web API belong to the game's developers —
  report them there.
- **Your own remote-exposure setup.** Exposing the companion
  or the game API beyond the guidance in
  [REMOTE-ACCESS.md](REMOTE-ACCESS.md) — for example with a
  public port forward — is your configuration, not a
  vulnerability in this software. Reports that the documented
  guidance itself is wrong or unsafe are in scope.

## The security model in brief

- **Local-first.** Superintendent serves its website on
  loopback only (`127.0.0.1`) by default, and there is no
  account and no login: the site just opens.
- **Nothing phones home.** No telemetry, no update checks, no
  hosted assets. The only outbound connection the toolkit ever
  makes is to the game endpoint you configure.
- **Local defence.** A loopback service can be talked to by
  other programs — and other web pages — on the same machine,
  so the site checks the `Host` header on every request and
  requires a per-form token on every POST.
- **Writes are deliberate.** Every change to the game goes
  through the prepare → confirm → audit flow
  ([dev/SPEC.md](dev/SPEC.md) §12): nothing applies without an
  explicit confirmation against a short-lived single-use
  token, and every applied action lands in the audit trail.
  Blueprint placements and terrain designations additionally
  travel as sealed QoSBuilder job files — single-use,
  expiring, pinned to one saved game, hash-checked.
  CoiBridge itself never writes to the game, and every QoS
  feature ships switched off.
- **Remote access is yours to run.** Running the companion on
  another machine is supported only over a private tunnel you
  control, as described in
  [REMOTE-ACCESS.md](REMOTE-ACCESS.md). Never expose the
  game's API port or the companion site on the public
  internet, and do not enable the game's
  `allowOtherOriginsPost` option.

## Disclosure

This is a solo-maintainer hobby project. Reports are read and
taken seriously, and fixes for genuine issues ship as
fast-track releases on a best-effort basis — but no
response-time or fix-time commitments (SLAs) are promised.
In return, reporters are asked to give the maintainer a
reasonable window to fix and release before any public
disclosure. Coordinated disclosure is appreciated.
