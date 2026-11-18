"""Evaluate the profile's depth graph at representative climate coordinates."""
import json
from functools import cache
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DENSITY = ROOT / "profiles/cleanroom_extreme_v1/data/minecraft/worldgen/density_function"
PREFIX = "minecraft:cleanroom_extreme/"


@cache
def load(name):
    return json.loads((DENSITY / (name.removeprefix("minecraft:") + ".json")).read_text())


def evaluate(node, y, climate):
    if isinstance(node, (int, float)):
        return node
    if isinstance(node, str):
        return climate[node] if node in climate else evaluate(load(node), y, climate)
    kind = node["type"].removeprefix("minecraft:")
    if kind == "cache":
        return evaluate(node["input"], y, climate)
    if kind == "spline":
        return spline(node["spline"], y, climate)
    if kind == "clamp":
        return min(node["max"], max(node["min"], evaluate(node["input"], y, climate)))
    if kind == "add":
        return evaluate(node["left"], y, climate) + evaluate(node["right"], y, climate)
    if kind == "y_clamped_gradient":
        t = min(1, max(0, (y - node["from_y"]) / (node["to_y"] - node["from_y"])))
        return node["from_value"] + t * (node["to_value"] - node["from_value"])
    raise ValueError(f"Unexpected terrain depth node {kind}")


def spline(node, y, climate):
    if isinstance(node, (int, float)):
        return node
    x = evaluate(node["coordinate"], y, climate)
    points = node["points"]
    for a, b in zip(points, points[1:]):
        if a["location"] <= x <= b["location"]:
            width = b["location"] - a["location"]
            t = (x - a["location"]) / width
            av = spline(a["value"], y, climate)
            bv = spline(b["value"], y, climate)
            return (
                (2*t**3 - 3*t**2 + 1)*av + (t**3 - 2*t**2 + t)*width*a["derivative"]
                + (-2*t**3 + 3*t**2)*bv + (t**3 - t**2)*width*b["derivative"]
            )
    end = points[0] if x < points[0]["location"] else points[-1]
    return spline(end["value"], y, climate) + (x - end["location"]) * end["derivative"]


def main():
    for label, coordinates, expected in (
        ("plain", (0.15, 0.45, 0.0), -400),
        ("mountain", (0.55, -0.65, 0.8), 400),
        ("deep ocean", (-0.75, 0.4, 0.0), -465),
    ):
        climate = dict(zip(
            ("minecraft:overworld/" + k for k in ("continents", "erosion", "ridges_folded")),
            coordinates,
        ))
        lo, hi = -512.0, 512.0
        for _ in range(40):
            mid = (lo + hi) / 2
            if evaluate(PREFIX + "depth", mid, climate) > 0:
                lo = mid
            else:
                hi = mid
        crossing = (lo + hi) / 2
        print(f"{label}: depth=0 at Y={crossing:.3f}")
        assert abs(crossing - expected) < 1, (label, crossing, expected)


if __name__ == "__main__":
    main()
