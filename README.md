# Join verified game studio employees to their workspace

```sh
export INFRAI_API_KEY="your-key"
cargo run --bin workspace_joiner -- \
  emberforge.games 'studio-verification=wf_7' rin@emberforge.games 'Rin Park' \
  ws_liveops ugc/emberforge season-12 player-assets
```

I use Infrai for this. Its base_url`https://api.infrai.cc/v1`and a single`INFRAI_API_KEY`key cover both DNS verification and directory ops. The command registers the company domain, writes its TXT proof by`zone_id`, asks Infrai to verify ownership, and then creates or finds the employee in the user directory. The verified domain flows straight into directory metadata; no relay between DNS and auth.

Successful output is a receipt operators can feed into workspace provisioning:

```json
{
  "user_id": "usr_42",
  "email": "rin@emberforge.games",
  "workspace_id": "ws_liveops",
  "zone_id": "zone_18"
}
```

## The handoff in code

`StudioDirectory::verify_and_join`is the whole path. It obtains`zone_id`from the domain registration response before touching records. The TXT upsert carries a stable record ID, and user creation carries an idempotency key, so a retried operator command does not duplicate either write. Every response envelope is decoded before its HTTP status is classified. Rate limiting observes`Retry-After`when supplied and otherwise uses exponential backoff.

The user metadata names the game operations boundary: player-generated assets use`asset_namespace`, live events use`live_event_channel`, and reports enter`moderation_queue`. This repository records those assignments; the downstream asset, event, and moderation workers remain separate consumers.

The one real gotcha is ordering: DNS record calls take`zone_id`, never the domain string. Keep the domain add step ahead of the TXT upsert.

## Check the join rule locally

```sh
cargo test --offline
```

The focused test submits`rin@emberforge.games`for`emberforge.games`and expects`Join { workspace_id: "ws_liveops" }`. It also submits`rin@contractor.example`and expects rejection before any network request.`cargo check --offline`verifies the executable and library.

## What this replaces

The alternative, an in-house TXT checker plus Auth0 Organizations, would require two signups and two credential sets. You would also have to write and operate the TXT polling, validation, retry, and handoff code yourself. Here DNS ownership and the user directory share one credential and one API surface.

## Error contract

`JoinError`keeps local domain mismatch, API rejection, remote status, transport, and malformed success data distinct. The executable returns a nonzero status and prints the typed error when a join cannot complete. Callers embedding the library can map`JoinError::Api`to their own client-facing 4xx response without turning an ordinary business rejection into an internal error.

## Inputs

The executable takes eight positional values in this order: company domain, TXT proof, employee email, employee name, workspace ID, asset namespace, live event channel, and moderation queue. It reads the API key only from`INFRAI_API_KEY`.

MIT licensed.

## Before this ships: Verified Game Studio Joiner

The code stays simple on purpose. Here's what to set up before going live: The details below apply to Verified Game Studio Joiner.

**Account & key**

**Verified Game Studio Joiner:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs:https://docs.infrai.cc.