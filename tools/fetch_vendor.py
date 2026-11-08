"""Download the JavaScript libraries Atlas Hunt uses (pinned versions) so the app never calls a CDN at runtime.

Usage:  python3 tools/fetch_vendor.py
Skips files that already exist, so running it again does nothing. Writes web/vendor/.
Licences: three.js MIT, d3 ISC, topojson-client ISC.
"""
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "web" / "vendor"
THREE = "https://cdn.jsdelivr.net/npm/three@0.160.0/"
# three.js add-ons keep their examples/jsm layout, because they import each other by relative path.
FILES = {
    "three/three.module.js": THREE + "build/three.module.js",
    "three/LICENSE": THREE + "LICENSE",
    "three/addons/controls/OrbitControls.js": THREE + "examples/jsm/controls/OrbitControls.js",
    "three/addons/modifiers/TessellateModifier.js": THREE + "examples/jsm/modifiers/TessellateModifier.js",
    "three/addons/postprocessing/EffectComposer.js": THREE + "examples/jsm/postprocessing/EffectComposer.js",
    "three/addons/postprocessing/Pass.js": THREE + "examples/jsm/postprocessing/Pass.js",
    "three/addons/postprocessing/MaskPass.js": THREE + "examples/jsm/postprocessing/MaskPass.js",
    "three/addons/postprocessing/ShaderPass.js": THREE + "examples/jsm/postprocessing/ShaderPass.js",
    "three/addons/postprocessing/RenderPass.js": THREE + "examples/jsm/postprocessing/RenderPass.js",
    "three/addons/postprocessing/UnrealBloomPass.js": THREE + "examples/jsm/postprocessing/UnrealBloomPass.js",
    "three/addons/postprocessing/OutputPass.js": THREE + "examples/jsm/postprocessing/OutputPass.js",
    "three/addons/shaders/CopyShader.js": THREE + "examples/jsm/shaders/CopyShader.js",
    "three/addons/shaders/LuminosityHighPassShader.js": THREE + "examples/jsm/shaders/LuminosityHighPassShader.js",
    "three/addons/shaders/OutputShader.js": THREE + "examples/jsm/shaders/OutputShader.js",
    "d3/d3.min.js": "https://cdn.jsdelivr.net/npm/d3@7.9.0/dist/d3.min.js",
    "d3/LICENSE": "https://cdn.jsdelivr.net/npm/d3@7.9.0/LICENSE",
    "topojson/topojson-client.min.js": "https://cdn.jsdelivr.net/npm/topojson-client@3.1.0/dist/topojson-client.min.js",
    "topojson/LICENSE": "https://cdn.jsdelivr.net/npm/topojson-client@3.1.0/LICENSE",
}


def main() -> int:
    downloaded = 0
    try:
        for relative, url in FILES.items():
            target = VENDOR / relative
            if target.exists():
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with urllib.request.urlopen(url, timeout=60) as response:
                target.write_bytes(response.read())
            downloaded += 1
    except OSError as error:
        print(f"error: could not download {url}: {error}", file=sys.stderr)
        return 1
    print(f"Vendor: {len(FILES)} files in web/vendor/, {downloaded} new files downloaded")
    return 0


if __name__ == "__main__":
    sys.exit(main())
