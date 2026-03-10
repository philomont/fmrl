#!/usr/bin/env python3
"""Sync theme palettes from fmrl.toml to docs/themes.json"""

import tomllib
import json
import sys
from pathlib import Path

def sync_themes():
    # Read fmrl.toml
    toml_path = Path(__file__).parent / "fmrl.toml"
    with open(toml_path, "rb") as f:
        config = tomllib.load(f)

    # Extract theme palettes
    themes = {}
    for theme_name, theme_data in config.get("themes", {}).items():
        themes[theme_name] = {
            "name": theme_data.get("name", theme_name.capitalize()),
            "ink": theme_data.get("ink", [0, 0, 0]),
            "paper": theme_data.get("paper", [230, 220, 195]),
            "accent": theme_data.get("accent", [180, 30, 30]),
            "highlight": theme_data.get("highlight", [255, 255, 255]),
        }

    # Write to docs/themes.json
    json_path = Path(__file__).parent / "docs" / "themes.json"
    with open(json_path, "w") as f:
        json.dump(themes, f, indent=2)

    print(f"✓ Synced {len(themes)} themes to {json_path}")
    for name in themes:
        print(f"  - {name}: {themes[name]['name']}")

if __name__ == "__main__":
    sync_themes()
