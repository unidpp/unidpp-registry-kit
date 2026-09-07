# ONBOARDING — joining the DPP registry federation as a jurisdiction

Ceremony guide for a country (national standards body, ministry, or
mandated operator) that operates — or plans to operate — a national
Digital Product Passport registry and wants it to interoperate as a
**federation peer** rather than as a tenant of someone else's root.

Companion documents: the strategy memo *National DPP Registries: where
they fit, and how we respond* (F1–F6 decomposition, the peer formula)
and PLAN-OPERATORS §3 (federation charter: tiers, cross-rights,
continuity, distrust). The kit in this repository is the reference
deployment the ceremony trains on.

## 0. The position you are onboarding into

A national registry is a first-class **peer**, never the root. Your
registry keeps your identifiers, your crypto, your surveillance access.
What the federation supplies in return is the interop protocol: signed
service discovery, dated applicability with as-of semantics, and a
multi-witnessed trust-anchor list that no single country controls. The
ePassport precedent: every state issues its own passports and inspects
everyone else's; ICAO Doc 9303 supplies the interop and the PKD supplies
a passive distribution of public keys — a tiny governance table, not a
world database of travel documents.

Membership tiers (PLAN-OPERATORS §3):

| Tier | Name | What it grants | Ceremony |
|---|---|---|---|
| T1 | Read federation | consume the registry + trust bundles | none |
| T2 | Service federation | register services (C3) with signed descriptors; continuity obligations | steps 1, 3, 5 below |
| T3 | Trust federation | your trust authority joins the master list (M-of-K) | step 2 |
| T4 | Governance | registrar/control-body seats under ISO 19135 roles; dispute panel | charter §3 |

## 1. Operator credential issuance (before anything is signed)

Every T2 act is a **signed service descriptor**. The signing key is an
operator credential issued by the trust service (PLAN-OPERATORS §5:
SIGNATIF realized as Confium — threshold-signed operator keys, so no
single employee's laptop is a single point of compromise).

- Request issuance through your jurisdictional trust authority (a
  threshold group, e.g. 3-of-5; algorithm suite per your profile —
  SM2/FIPS/ML-DSA co-signatures are all expressible).
- The credential binds: operator id (content-derived from the public
  key), key id, public key, algorithm. One operator id pins exactly one
  key — cross-key forgeries fail content-derivation checks.
- **In this kit (development):** the descriptor in `bin/seed-jurisdiction.py`
  is signed with a deterministic dev key from the unidpp-registry dev
  keyring (`SHA-256("UNIDPP-DISCOVERY/OPERATOR-SEED" || label)`), which
  the reference registry trusts at intake. The production seam is
  `AppState::keyring` in unidpp-registry: replace the dev keyring with
  your trust list (the trust service's signed trust-list endpoint) and
  the same code path verifies your Confium-issued credential.
- Record the credential in your key ceremony log; rotation and
  revocation are reason-coded, windowed trust events, never silent.

## 2. Trust-list entry

Your jurisdictional trust authority must be discoverable before peers
will act on your signatures.

- Operate (or join) a **jurisdictional trust authority** — threshold
  group, not a single key.
- Publish a signed **trust list** through the trust service
  (`GET /trust-lists`), listing the operator credentials your
  jurisdiction vouches for, with revocation windows.
- For T3: apply for **master-list membership**. The master list is
  M-of-K co-signed by independent log operators; admission, rotation
  and ejection are threshold ceremonies. Deliberately no single country
  controls it.
- Register a C5 **verification mechanism** item (suite, agility status,
  trust-framework ref, trust-list endpoint, master-list ref) in your
  registry and in the federated discovery layer, so any client can
  assemble the verification bundle for your jurisdiction mechanically.

## 3. Discovery self-registration (C3)

Register your registry as a service descriptor — class `registry`,
jurisdiction set to your ISO 3166-1 alpha-2 code. This is exactly what
`bin/seed-jurisdiction.py --jurisdiction <CODE>` does against your
deployment:

```sh
bin/run-registry.sh --daemon
bin/seed-jurisdiction.py --jurisdiction DE
curl -s 'http://127.0.0.1:8391/services?jurisdiction=DE&class=registry' | jq
```

The descriptor body carries:

- `operator` — the credential block from step 1;
- `class: "registry"` with the jurisdictional-registry marker (memo R6
  formalizes the `jurisdictional-registry` C3 subclass);
- `endpoints[]` — where peers reach you, each with a **protocol
  binding ref** (C4; e.g. the EN 18222 REST shape) so "whose wire
  grammar?" is answered by a registered item, not by prose;
- `jurisdiction`, `residency_class`, `status`;
- `succession_pointer` — null until a succession is filed (step 5).

Admission at T2 = your signed descriptor is accepted, you publish under
your own operator credential and trust list, and you accept the
continuity obligations. Nobody can eject your jurisdiction from
*discovering* or *being discovered*; suspension is a trust event, not a
hosting event.

## 4. Jurisdiction profile, applicability, semantics (F4)

Your jurisdiction's requirements become registered, versioned items —
not PDFs on a ministry website:

1. **National semantic subregister**: run your register
   (`jurisdiction-<code>`) as a FERIN-style sibling register under
   ISO 19135:2025 discipline. Data elements are items with definitions,
   units, representations; cross-register mappings to common concepts
   (OpenCDD/CDD, UNTDED heritage) are themselves registered items with
   attesters — that is how national elements harmonize instead of
   colliding. (EPSG is the precedent: a national-origin register that
   became the global reference by being register-disciplined, not by
   being declared universal.)
2. **Jurisdiction profile item**: the profile your delegated acts
   demand — data points referencing the semantic items, custody model
   (ISO 22095), legal basis, trigger predicates.
3. **Applicability bindings**: dated, identity-keyed bindings of
   profile to product types — including **retroactive** bindings when
   the law backdates (end-of-waste style re-qualification). Every
   binding is replayable as-of: "what applied to this product on that
   date?" is always answerable, which is what makes retroactivity
   auditable instead of arbitrary.

## 5. Continuity and succession filing

At T2 admission you file a succession plan; it is a conformance
obligation, not paperwork.

- Register the plan with the federation registrar: escrow arrangements
  (EN 18221 back-up pattern), resolver-redirection steps, handover
  ceremony for hosting, and the identity rule — **identities outlive
  issuers and hosts** (I14).
- On succession, set the descriptor's `succession_pointer` to the
  successor service and transition status to `succeeded` — clients
  follow the pointer mechanically.
- On failure (operator silent), the registrar's continuity marking and
  resolver redirection take over; a jurisdiction's installed base must
  never become unverifiable because one operator did.
- Deposit obligations (memo F3), where your legislation demands copies:
  notarized Tier-C snapshots at a national archivist, under your
  profile — and **republication never transfers authority** (I7).

## Conformance checklist — "jurisdictional registry operator"

The conformance class a national registry must satisfy to interoperate
(work item R2 carries this into the specification). Each row shows how
the reference deployment in this kit demonstrates it.

| # | Requirement | Demonstrated by |
|---|---|---|
| 1 | **ISO 19135 governance**: submitting organization, control-body discipline, versioned items, supersession with reason + successor | `POST /data-elements`, `POST /items/{id}/versions` — see demo step 5 |
| 2 | **Signed descriptors**: all service descriptors signed by an operator credential verifiable against a published trust list | demo step 3 (Ed25519 over canonical JSON, content-derived operator id) |
| 3 | **Enumeration resistance**: no surface lists identifiers-in-use; applicability is identity-keyed; audit log is guarded | demo step 6 |
| 4 | **As-of semantics**: every read is point-in-time reconstructable; retroactivity is explicit and dated | demo step 4 (`?at=`), `retroactive` bindings |
| 5 | **Mirroring**: state is an append-only, replayable journal; snapshots are cacheable because versions are immutable | `data/jurisdiction-journal.jsonl` replayed on restart |
| 6 | **Continuity/succession**: succession plan filed; pointer + `succeeded` status; continuity marking | descriptor `succession_pointer` (step 5 above) |
| 7 | **No payload custody**: the registry holds descriptors and bindings, never passport payloads | nothing in the route table stores passports; deposit is Tier-C notarization |
| 8 | **Jurisdiction-scoped service class**: C3 descriptor with `class=registry`, jurisdiction set, protocol binding refs | `seed-jurisdiction.py` step 1 |

## What the kit does not do for you

- It does not issue keys fit for production (dev keyring — replace at
  the documented seam).
- It does not operate the trust service or master list (see
  unidpp-trust / PLAN-OPERATORS §5).
- It does not legislate: applicability bindings encode your law's
  timing; they do not create it.
- It does not host your identifiers-in-use index — if your legislation
  requires one, keep it behind predicate access for entitled
  regulators, never browsable, per memo §3.3.
