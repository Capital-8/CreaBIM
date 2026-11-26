"""Reference geometry for the IFC bench, made by IfcOpenShell.

IfcOpenShell is the engine Bonsai draws with, so its meshes are how Bonsai
sees a file: every element with its booleans, clips and mapped items
applied. For each element with a body this writes its GlobalId, class,
world box (metres), volume (cubic metres) and triangle count as JSON, for
`creabim ifc-check <file.ifc> --reference <file.ref.json>` to compare
CreaBIM's own geometry with, element by element.

    python reference.py Highland_Haven.ifc Highland_Haven.ref.json

IfcOpenShell is not part of CreaBIM; it is only needed to make this file.
"""
import json
import multiprocessing
import sys

import ifcopenshell
import ifcopenshell.geom


def volume(verts, faces):
    v = 0.0
    for i in range(0, len(faces), 3):
        a, b, c = faces[i], faces[i + 1], faces[i + 2]
        ax, ay, az = verts[3 * a : 3 * a + 3]
        bx, by, bz = verts[3 * b : 3 * b + 3]
        cx, cy, cz = verts[3 * c : 3 * c + 3]
        v += ax * (by * cz - bz * cy) - ay * (bx * cz - bz * cx) + az * (bx * cy - by * cx)
    return v / 6.0


def main(src, out):
    f = ifcopenshell.open(src)
    s = ifcopenshell.geom.settings()
    s.set("use-world-coords", True)
    it = ifcopenshell.geom.iterator(s, f, multiprocessing.cpu_count())
    rows = {}
    failed = 0
    if it.initialize():
        while True:
            shape = it.get()
            v = shape.geometry.verts
            fc = shape.geometry.faces
            if v:
                xs, ys, zs = v[0::3], v[1::3], v[2::3]
                rows[shape.guid] = {
                    "class": f.by_guid(shape.guid).is_a(),
                    "lo": [min(xs), min(ys), min(zs)],
                    "hi": [max(xs), max(ys), max(zs)],
                    "volume": abs(volume(v, fc)),
                    "tris": len(fc) // 3,
                }
            if not it.next():
                break
    else:
        failed = 1
    with open(out, "w", encoding="utf-8") as o:
        json.dump(
            {"source": src, "engine": "IfcOpenShell " + ifcopenshell.version, "elements": rows},
            o,
        )
    print(f"{len(rows)} elements with a body written to {out}")
    return failed


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
