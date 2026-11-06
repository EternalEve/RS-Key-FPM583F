#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Generate a millimetre SVG cutting template for the V11 silicone pads."""

from trial_model import ROOT, parameters


def write_template(p, version, output):
    outer, inner = p["sensor_pad_outer"], p["sensor_pad_free_inner"]
    width, height = p["rear_pad_width"], p["rear_pad_height"]
    strip_length, strip_width = p["top_pad_length"], p["top_pad_width"]
    content = f'''<?xml version="1.0" encoding="UTF-8"?>
<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2026 RS-Key contributors -->
<svg xmlns="http://www.w3.org/2000/svg"
     width="85mm" height="50mm" viewBox="0 0 85 50">
  <title>{version} silicone pads; mm; cut only group cut; no scaling</title>
  <g id="cut" fill="none" stroke="#000" stroke-width="0.05">
    <circle cx="14" cy="19" r="{outer/2}"/>
    <circle cx="14" cy="19" r="{inner/2}"/>
    <rect x="36" y="12" width="{width}" height="{height}"/>
    <rect x="43" y="12" width="{width}" height="{height}"/>
    <rect x="53" y="9" width="{strip_length}" height="{strip_width}"/>
  </g>
  <g id="notes-do-not-cut" fill="#555" font-family="sans-serif" font-size="2.1">
    <text x="3" y="4">{version} / mm / 1:1 / CUT group only / DO NOT SCALE</text>
    <text x="3" y="33">Ring: OD {outer} / ID {inner}</text>
    <text x="34" y="26">2 x {width} x {height}</text>
    <text x="51" y="31">1 x {strip_length} x {strip_width}</text>
    <text x="3" y="38">Solid silicone: HT-6240 / 40A / {p['pad_free_thickness']} mm nominal</text>
    <text x="3" y="42">4 cut pieces. No adhesive thickness included.</text>
    <text x="56" y="46">10 mm reference</text>
    <path d="M 57,41 v 2 M 57,42 h 10 M 67,41 v 2" stroke="#555" fill="none" stroke-width="0.15"/>
  </g>
</svg>
'''
    output.write_text(content, encoding="utf-8")
    print(output)


def main():
    write_template(parameters(), "V11", ROOT / "build/v11-silicone-pads-1to1.svg")


if __name__ == "__main__":
    main()
