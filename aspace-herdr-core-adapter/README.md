# Adapter A'Space — herdr-core (D-N053 pas F1)

**Adresse :** `fiber:a0:herdr-core`
**Manifeste :** `herdr-core.manifest.json` (domicile V5 `01_Forkable/FKB00_Runtime/00_A0_Core/`)
**Audit télémétrie :** `herdr-core-TELEMETRY-AUDIT.md` (même domicile)
**Statut :** état 1 Adapté (pas F1, PR V5 à juger par le 13e)
**Licence :** Apache-2.0

## Commit gelé

- **Fork :** `Amdkn/herdr-core` (parent `herdrdev/herdr-core`, Apache-2.0, branche `master`).
- **Gel : `cd12ddd`** — version 0.1.18 + 10 commits (`v0.1.18-10-gcd12ddd`), 2026-11-07
  (« fix: make restart environment handoffs best effort and single-use »).
- **Tout commit amont ultérieur exige un amendement de design**, jamais un
  fast-forward silencieux (D-F1-4). Dérive au gel : 53 commits vers l'amont
  HEAD `13da068` (v0.1.19 `06793ed` + v0.1.20 `de97ea5`) — triage versé au
  design §2.1 : protocole gelé (0 commit sur `wire.rs` + `rpc.rs`), schéma
  additif seul (32 méthodes = 32 méthodes, `server.restart` gagne un paramètre
  optionnel `environment`), `server.rs` +90 lignes, `client.rs` +26,
  `lib.rs` +7, 84 fichiers touchés. Revisite par signal raccourcie à 3 jours
  tant que la dérive dépasse 20 (D-F1-4, suivi S-F1-3).

## Adapter

- **Dossier :** `aspace-herdr-core-adapter/` (crate adapter + contrat JSON +
  config embarquée + preuve de protocole + smoke test shell), commit adapter
  sur le fork. **Seul code A'Space du fork** — le code amont est
  byte-identique au gel (H-F1-2).
- **Forme (D-F1-1, les deux) — une vraie crate, pas un contrat seul :**
  - **Bibliothèque** (`src/lib.rs`, crate `aspace-herdr-core-adapter`) :
    ré-exporte **33 des 35 modules `pub`** de `lib.rs` amont — tous sauf
    `server` (le daemon `server::serve()`) et `cli` (`cli::main()` dispatche
    `server start`, `update`, `serve`, `hold` et auto-démarre un daemon).
    Aucun code A'Space ne peut donc atteindre le serveur par cette crate.
  - **CLI cliente** (`src/bin/hc.rs`, binaire `hc`) : chaque verbe `hc`
    parle à un core **déjà en cours, côté opérateur**, via
    `client::require_existing` — jamais `connect_or_start`. `hc server
    start`, `hc update`, `serve`, `hold` n'existent pas dans ce binaire :
    clap les rejette avant toute exécution. Verbes : `run ls get kill rm
    label keep send wait keys interrupt attach read put exec`,
    `terminal <verbe>`, `file read|write|edit`, `machine add|ls|login|
    update|rename|rm`, `events`, `templates`, `server status|stop|restart`,
    `schema`, `doctor`, `call <méthode>`.
  - **Protocole** : la connexion démarre en mode plain (une ligne JSON-RPC
    par ligne) ; un `hello` réussi avec `framed: true` bascule en trames
    (`server.rs`, en-tête vérifié) ; canal 0 = JSON-RPC ; schéma
    `schema/hc.openrpc.json` (32 méthodes, versionné comme contrat). Les
    appels unitaires de `client::call` restent en mode plain — c'est le
    mode encadré qui porte les flux attachés. Précision vérifiée au code :
    le décompte « 30 modules » du design est périmé — `lib.rs` amont porte
    35 modules `pub` au gel (dont `server` et `cli`, exclus de la surface) ;
    la crate en ré-exporte 33 ; le contrat liste les 35.
- **Jamais (le serveur `hc` n'est PAS réintroduit) :** `server::serve()`
  (verrou `core.lock`, écoute, `Core::start`, `recover_kept`, `run_jobs`,
  `watch_foreground`, `save_when_due`, enregistrements launchd/systemd) ;
  `hc server start` ; l'état `~/.local/state/herdr-core/` tenu par un serveur
  V5 ; l'auto-démarrage (`demand-start`, `hc-launchd-*`, `connect_or_start`
  — le client adapter n'utilise que `connect_existing` /
  `require_existing`, jamais `connect_or_start`) ; `hc update` (jamais contre
  `releases.herdr.dev` — les tests amont utilisent le faux serveur local
  `releases_server.py`).
- **Socket par défaut réel (E-F1-4) :** `$XDG_RUNTIME_DIR/herdr-core/default.sock`
  (session `default`, env `HC_SESSION` ; override `HC_SOCKET` > config >
  défaut). `/tmp/herdr-core.sock` n'est PAS le défaut `hc` — c'était
  l'override du superviseur V5, fibre retirée par F8 pas 8.
- **Acte de build (E-F1-8) :** le sous-module `vendor/ghostty` est non
  initialisé en clone frais (shallow `-1e5cdb8`) — la fiche inscrit
  `git submodule update --init --depth 1` comme acte de build, pas de code.
- **Interrupteur forcé (D-F1-5, H-F1-4) :** `notes = false` — config embarquée
  `aspace-herdr-core-adapter/config.toml`, vérifiée au fichier ET par la
  preuve de protocole (la lib lit la config et `notes == Some(false)`).
  Défaut amont : ON (`config.rs` : « Notes to parent agents […] on or off
  (default on) »).
- **Couture herdr ↔ herdr-core (E-F1-3) :** 0 lien de code inventé — les deux
  repos sont deux produits séparés à ces pins (0 occurrence de herdr-core
  dans herdr au gel). La couture nommée est (a) la bibliothèque adapter
  herdr-core comme client du protocole encadré `hc`, (b) l'adapter
  CLI-contrat herdr comme client du socket herdr NDJSON. Le natif consomme
  les deux voies séparément (D-F1-2).
- **Revendication 31415 (D-F1-6) :** le bind revendique
  `universal_harness_gateway_31415` +
  `http://127.0.0.1:31415/v1/chat/completions` — qualifiée côté A'Space
  (FreeCloud, `aspace_freecloud_gateway.py`), pas une capacité du gel
  (herdr ne sert rien sur 31415 au gel). Bind re-appliqué verbatim, non
  modifié au pas F1.
- **Preuve client du protocole encadré (H-F1-3) — deux angles :**
  `tests/protocol_roundtrip.rs` (suite de la crate : round-trip trames
  `hello` sur le canal 0, round-trip plain-mode `client::call`
  `server.status`, constantes, socket par défaut) et `protocol-proof/`
  (binaire autonome : la lib lit la config adapter et `notes ==
  Some(false)`, `require_existing` échoue proprement sans serveur, puis
  `hello` plain avec `framed: true` suivi d'un round-trip trames +
  JSON-RPC `server.status` sur le canal 0 contre un stub local). Jamais
  contre un serveur `hc` V5, jamais contre `releases.herdr.dev`.
- **Smoke test :** `bash aspace-herdr-core-adapter/smoke_test.sh` (crate
  adapter debug, session scratch `HC_SESSION=aspace-f1-smoke`, XDG scratch ;
  construit la crate, lance ses tests, vérifie le binaire adapter `hc`
  (`--version`, `--help`, `doctor`, `server status` sans serveur =
  indisponible, sans auto-démarrage), la config `notes = false` lue par la
  lib, l'absence de `server`/`update` de la surface du contrat, et la
  preuve de protocole).

## Tests au gel et sur le fork adapté

- **Au gel `cd12ddd` :** voir
  `~/workspace/v4-factory/evidence/D-N053/f1/herdr-core/logs-suite-baseline-gel.log`
  (`cargo test --locked` workspace Linux + `fmt` + `clippy -- -D warnings`).
  Tests `windows`/`macos` exclus (cibles CI par matrice, pas cette VM).
  `cargo-deny` exclu nommément (absent de la VM — lint, pas gate).
- **Sur l'adapté :** même suite + smoke adapter vert + preuve de protocole
  verte, sorties consignées sous
  `~/workspace/v4-factory/evidence/D-N053/f1/herdr-core/`.

## Télémétrie

**Zéro appel sortant par défaut.** Voir `herdr-core-TELEMETRY-AUDIT.md` :
aucun SDK de télémétrie, aucune crate HTTP (`reqwest`/`ureq`/`hyper`/`curl`
absents du `Cargo.lock`) ; tout le réseau passe par `curl` en sous-processus
ou par SSH, rien n'émet sans commande explicite, sauf : `hc update`
(manuel, hors contrat — jamais automatique : « hc never checks for updates
on its own », `cli.rs:591`) et `notes` agent-à-agent (défaut ON, forcé OFF
par l'adapter). Machines distantes : opt-in explicite (`hc machine add/
connect`, `machine.exec`), SSH, pas de télémétrie.

## Divergence (dette portée par le cœur Buzz, lane Clara / 12e)

- **Règles maison amont respectées (AGENTS.md) :** la racine `AGENTS.md`
  amont n'a PAS été éditée (H-M2) ; l'adapter ne touche pas au cœur ;
  aucune modification du comportement agent.
- **Ré-audit télémétrie :** à chaque changement de commit gelé ou d'adapter,
  et au minimum à chaque transition d'état (D3).
- **Dette de divergence :** 53 commits amont non repris (triage versé au
  design §2.1) ; revisite par signal à 3 jours tant que la dérive dépasse 20.

## Références

- Design pas F1 signé Clara (2026-11-12) :
  `~/workspace/v4-factory/plans/DESIGN-D-N053-PAS-F1-PROJET.md`.
- Discover pas F1 de Bill :
  `~/workspace/v4-factory/evidence/D-N053/f1/DISCOVER-PAS-F1.md`.
- Évidences : `~/workspace/v4-factory/evidence/D-N053/f1/herdr-core/`.
- Fork : https://github.com/Amdkn/herdr-core (gel `cd12ddd`, adapter — voir
  manifeste).
