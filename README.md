# QuikFlip

Four daily games in one self-hosted app. See [DESIGN.md](DESIGN.md) for how it fits together.

## Run it on your laptop (Fedora or Windows)

Needs Rust (https://rustup.rs).

```sh
cargo test                 # unit tests
cargo run -- serve         # http://localhost:8080
cargo run -- generate      # make missing Rarefind puzzles once (uses the Claude Code you are logged into)
```

Run `serve` and `generate` with the same `QUIKFLIP_DATA_DIR` so they share one database. `QUIKFLIP_LANGUAGES=en` limits generation to one language while testing.

Settings come from environment variables (all optional):

| Variable | Default | Meaning |
|---|---|---|
| `QUIKFLIP_DATA_DIR` | `data/` next to the binary | Where `quikflip.sqlite3` lives |
| `QUIKFLIP_WEB_DIR` | `web/` next to the binary, else the source tree's `web/` | The front end |
| `QUIKFLIP_BIND` | `0.0.0.0:8080` | Address and port |
| `QUIKFLIP_TIMEZONE` | `Europe/Brussels` | When a new day starts |
| `QUIKFLIP_LANGUAGES` | `en,fr,nl` | Languages the generator makes puzzles for |
| `QUIKFLIP_CLAUDE_BIN` | `claude` | The Claude Code CLI |
| `QUIKFLIP_CLAUDE_MODEL` | the CLI's default | Model alias, e.g. `opus` or `sonnet` |
| `QUIKFLIP_CLAUDE_TIMEOUT_SECONDS` | `900` | Longest a single Claude call may take |
| `CLAUDE_CODE_OAUTH_TOKEN` | none | Subscription token for the generator container (from `claude setup-token`) |

## Put it on TrueNAS SCALE

Needs TrueNAS SCALE 24.10 or newer (apps run on Docker) and Podman on your laptop (`sudo dnf install podman` on Fedora; Podman Desktop on Windows).

1. Build the image and save it to a file:
   ```sh
   podman build -t localhost/quikflip:0.2.0 .
   podman save -o quikflip-0.2.0.tar localhost/quikflip:0.2.0
   ```
2. Copy `quikflip-0.2.0.tar` to the NAS (an SMB share works) and load it in the TrueNAS shell (System > Shell):
   ```sh
   sudo docker load -i /mnt/tank/path/to/quikflip-0.2.0.tar
   ```
3. Create a dataset for the data, for example `tank/apps/quikflip`, with the "Apps" preset so user 568 owns it.
4. On your laptop, run `claude setup-token` and copy the token it prints. It is tied to your Claude subscription; treat it like a password.
5. Apps > Discover Apps > the three-dot menu > Install via YAML. Paste `compose.yaml` after changing `/mnt/tank/apps/quikflip` (twice) to your dataset and pasting the token into `CLAUDE_CODE_OAUTH_TOKEN`.
6. Open `http://<nas-address>:8080`. The generator makes the first Rarefind puzzles within a few minutes; its log is under the app's `quikflip-generator` container.

To update: build with a new version tag, load it, change the tag in the app's YAML. The database migrates itself on start; take a TrueNAS snapshot of the dataset first.

## Word lists and fonts

Both are generated and committed, so normal builds need no network.

```sh
python3 tools/build_wordlists.py   # rebuild words/ (edit tools/blocklist-*.txt to remove words first)
python3 tools/fetch_fonts.py       # download web fonts into web/fonts/
```
