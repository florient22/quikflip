# QuikFlip

Four daily games in one self-hosted app. See [DESIGN.md](DESIGN.md) for how it fits together.

## Run it on your laptop (Fedora or Windows)

Needs Rust (https://rustup.rs).

```sh
cargo test                 # unit tests
cargo run -- serve         # http://localhost:8080
cargo run -- generate      # make missing puzzles once: Rarefind and Atlas Hunt with the Claude Code you are logged into, Snippet songs from Deezer
```

Run `serve` and `generate` with the same `QUIKFLIP_DATA_DIR` so they share one database. `QUIKFLIP_LANGUAGES=en` limits generation to one language while testing. The generator needs internet access (Claude and Deezer); the web server does not. Atlas Hunt needs a browser with WebGL.

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

The code lives in a private GitHub repository (`florient22/quikflip`). Pushing a version tag makes GitHub Actions build the image and publish it, privately, as `ghcr.io/florient22/quikflip`; the NAS pulls it from there. Needs TrueNAS SCALE 24.10 or newer (apps run on Docker).

### Publish a version

1. Set the new version in `Cargo.toml` and in both `image:` lines of `compose.yaml`, then commit.
2. Tag and push:
   ```sh
   git tag v0.3.4
   git push && git push --tags
   ```
3. Watch the build under the repository's Actions tab (about 10 minutes the first time, a few minutes after that). It fails on purpose if the tag and the `Cargo.toml` version differ. Private repositories get a monthly allowance of free Actions minutes, which is plenty for this.

### First install on the NAS

1. On GitHub: Settings > Developer settings > Personal access tokens > Tokens (classic) > Generate new token, with only the `read:packages` scope. This lets the NAS download the private image; treat it like a password.
2. Sign the NAS in to the registry. In recent TrueNAS versions: Apps > Configuration > the Docker registry sign-in option, with registry `ghcr.io`, user `florient22` and the token as password. If your version has no such option, use System > Shell instead:
   ```sh
   sudo docker login ghcr.io -u florient22
   ```
   and paste the token when asked.
3. On your laptop, run `claude setup-token` and copy the token it prints. It is tied to your Claude subscription; treat it like a password.
4. Apps > Discover Apps > the three-dot menu > Install via YAML. Paste `compose.yaml` after pasting the Claude token into `CLAUDE_CODE_OAUTH_TOKEN`. Docker creates the `quikflip-data` volume for the database by itself.
5. Open `http://<nas-address>:8090` (the first number under `ports:` in `compose.yaml`; change it if another app already uses that port). Snippet's songs are ready within a minute; Rarefind and Atlas Hunt take a few minutes per language (the very first run also writes a few hundred Atlas fun facts). The generator's log is under the app's `quikflip-generator` container.

### Web UI button and icon

The `x-portals` block in `compose.yaml` adds the "Web UI" button, but TrueNAS only reads it on install. If the app was installed without it: Apps > quikflip > Edit, copy the YAML (it holds your Claude token), delete the app, and install it again via YAML with that text plus the `x-portals` block.

Custom apps get no icon from the YAML. To show QuikFlip's own icon, run this in System > Shell while the app is running, then reload the Apps page (Ctrl+Shift+R). TrueNAS may reset it when you edit the app; run it again then.

```sh
sudo python3 - <<'EOF'
import base64, subprocess, yaml
svg = subprocess.run(["docker", "exec", "quikflip", "cat", "/app/web/icon.svg"], check=True, capture_output=True).stdout
icon = "data:image/svg+xml;base64," + base64.b64encode(svg).decode()
for path, app_keys in [("/mnt/.ix-apps/metadata.yaml", ["quikflip"]), ("/mnt/.ix-apps/app_configs/quikflip/metadata.yaml", [])]:
    try:
        with open(path) as file:
            data = yaml.safe_load(file)
        entry = data
        for key in app_keys:
            entry = entry[key]
        entry["metadata"]["icon"] = icon
    except (OSError, KeyError, TypeError) as error:
        print(f"skipped {path}: {error!r}")
        continue
    with open(path, "w") as file:
        yaml.safe_dump(data, file, sort_keys=False)
    print(f"icon set in {path}")
EOF
```

### Update

Publish a new version (above), then edit the app's YAML in TrueNAS and change the tag in both `image:` lines; TrueNAS pulls the new image and restarts the app. The database migrates itself on start and stays in the `quikflip-data` volume.

### Back up

The volume sits in TrueNAS's own Docker storage, so dataset snapshots don't cover it. To copy the database out, stop the app, run this in System > Shell (any folder on one of your datasets works), then start the app again:

```sh
sudo docker cp quikflip:/data /mnt/tank/backups/quikflip-$(date +%F)
```

Deleting the app can delete its volume too, and with it your history and streaks; back up first.

### Without GitHub

To build on your own machine instead (Podman: `sudo dnf install podman` on Fedora, Podman Desktop on Windows):

```sh
podman build -t ghcr.io/florient22/quikflip:0.3.4 .
podman save -o quikflip-0.3.4.tar ghcr.io/florient22/quikflip:0.3.4
```

Copy the file to the NAS, run `sudo docker load -i /mnt/tank/path/to/quikflip-0.3.4.tar` in System > Shell, and add `pull_policy: never` to both services so TrueNAS uses the loaded image instead of pulling.

## Word lists, country data, fonts and libraries

All are generated and committed, so normal builds need no network.

```sh
python3 tools/build_wordlists.py   # rebuild words/ (edit tools/blocklist-*.txt to remove words first)
python3 tools/build_countries.py   # rebuild Atlas Hunt's country list and map in web/games/atlas/data/
python3 tools/fetch_fonts.py       # download web fonts into web/fonts/
python3 tools/fetch_vendor.py      # download three.js, d3 and topojson-client into web/vendor/
```

The country list is compiled into the server too, so rebuild the binary after `build_countries.py`.
