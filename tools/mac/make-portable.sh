#!/usr/bin/env bash
# A portable quickRS folder for macOS: copy it to any Mac (USB drive, external disk) and open quickRS.app; settings,
# favourites, scores, licks and the songs travel with it (the app uses a "quickRS Data" folder beside quickRS.app
# when it has a config.json or userdata.json: qrs-library paths.rs). From the port that ran on the owner's Mac.
#
#   tools/mac/make-portable.sh [out-dir] [songs-dir] [data-dir]
#
# Defaults: out-dir ~/Desktop/quickRS Portable, songs-dir ~/Music/quickRS/songs, data-dir the Mac data folder.
# Layout made:
#   quickRS Portable/
#     quickRS.app
#     quickRS Data/   config.json (library_dirs ["../Songs"]), userdata.json, keys.json
#     Songs/
# keys.json is copied when there: don't share the folder publicly with it inside.
set -euo pipefail
cd "$(dirname "$0")/../.."

out="${1:-$HOME/Desktop/quickRS Portable}"
songs="${2:-$HOME/Music/quickRS/songs}"
data="${3:-$HOME/Library/Application Support/quickRS}"

[ -d dist/mac/quickRS.app ] || NO_DMG=1 bash tools/mac/bundle.sh

mkdir -p "$out/quickRS Data" "$out/Songs"
rm -rf "$out/quickRS.app"
ditto dist/mac/quickRS.app "$out/quickRS.app"
[ -d "$songs" ] && rsync -a --delete "$songs/" "$out/Songs/"
for f in userdata.json keys.json; do
    if [ -f "$data/$f" ]; then cp "$data/$f" "$out/quickRS Data/"; fi
done
# the settings, with the song folder relative to the data folder and the audio devices left to pick themselves
python3 - "$data/config.json" "$out/quickRS Data/config.json" <<'EOF'
import json, sys
try:
    c = json.load(open(sys.argv[1]))
except (OSError, ValueError):
    c = {}
c["library_dirs"] = ["../Songs"]
c.pop("input_device", None)
c["audio"] = {"input": {"mode": "auto"}, "channel": None, "output": {"mode": "auto"},
              "exclusive": False, "buffer": None, "use_rocksmith": False}
json.dump(c, open(sys.argv[2], "w"), indent=2)
EOF
du -sh "$out"
echo "portable folder: $out"
