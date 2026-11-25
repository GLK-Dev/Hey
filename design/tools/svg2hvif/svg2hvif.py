#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
svg2hvif — конвертер SVG → HVIF (Haiku Vector Icon Format) для HeOS.

Использование:
    python svg2hvif.py icon.svg                      # → icon.hvif рядом
    python svg2hvif.py design/icons -o build/hvif    # вся папка, структура сохраняется
    python svg2hvif.py icon.svg --format hvif,rdef,cpp,preview
    python svg2hvif.py --decode icon.hvif -o icon.preview.svg   # HVIF → SVG для проверки

Зависимости: только стандартный Python 3.8+.
Для <text> нужен fontTools (pip install fonttools) и шрифты из design/fonts.

Формат сверен с исходниками Haiku (MIT):
src/libs/icon/flat_icon/FlatIconFormat.cpp, FlatIconImporter.cpp, PathCommandQueue.cpp,
src/libs/icon/IconRenderer.cpp, src/apps/icon-o-matic/import_export/svg/DocumentBuilder.cpp.
Код ниже написан заново, исходники Haiku не копировались.
"""
import argparse
import math
import os
import re
import struct
import sys
import xml.etree.ElementTree as ET

VERSION = "1.0"
CANVAS = 64.0

# --- константы формата (FlatIconFormat.h) ---------------------------------
STYLE_SOLID, STYLE_GRADIENT, STYLE_SOLID_NO_ALPHA, STYLE_GRAY, STYLE_GRAY_NO_ALPHA = 1, 2, 3, 4, 5
SHAPE_PATH_SOURCE = 10
TR_AFFINE, TR_CONTOUR, TR_PERSPECTIVE, TR_STROKE = 20, 21, 22, 23
GF_TRANSFORM, GF_NO_ALPHA, GF_GRAYS = 1 << 1, 1 << 2, 1 << 4
PF_CLOSED, PF_COMMANDS, PF_NO_CURVES = 1 << 1, 1 << 2, 1 << 3
CMD_H, CMD_V, CMD_L, CMD_C = 0, 1, 2, 3
SF_TRANSFORM, SF_HINTING, SF_LOD, SF_TRANSFORMERS, SF_TRANSLATION = 1 << 1, 1 << 2, 1 << 3, 1 << 4, 1 << 5
GRAD_LINEAR, GRAD_CIRCULAR = 0, 1
# agg: line_join miter=0 round=2 bevel=3; line_cap butt=0 square=1 round=2
JOINS = {"miter": 0, "miter-clip": 0, "arcs": 0, "round": 2, "bevel": 3}
CAPS = {"butt": 0, "square": 1, "round": 2}
MAGIC = b"ncif"  # 'ficn' little-endian

# ==========================================================================
#  Матрицы: (a, b, c, d, e, f) как в SVG matrix() и agg::trans_affine
#  x' = a*x + c*y + e ;  y' = b*x + d*y + f
# ==========================================================================
IDENT = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0)


def mmul(m, n):
    """m ∘ n: сначала n, потом m."""
    a, b, c, d, e, f = m
    A, B, C, D, E, F = n
    return (a * A + c * B, b * A + d * B, a * C + c * D, b * C + d * D,
            a * E + c * F + e, b * E + d * F + f)


def mapply(m, p):
    return (m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5])


def mdet(m):
    return m[0] * m[3] - m[1] * m[2]


def minv(m):
    a, b, c, d, e, f = m
    det = a * d - b * c
    if abs(det) < 1e-12:
        return IDENT
    return (d / det, -b / det, -c / det, a / det, (c * f - d * e) / det, (b * e - a * f) / det)


def mscale(m):
    return math.sqrt(abs(mdet(m)))


NUM_RE = re.compile(r"[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?")


def nums(s):
    return [float(x) for x in NUM_RE.findall(s or "")]


def parse_transform(s):
    m = IDENT
    for name, args in re.findall(r"([a-zA-Z]+)\s*\(([^)]*)\)", s or ""):
        v = nums(args)
        t = IDENT
        if name == "matrix" and len(v) == 6:
            t = tuple(v)
        elif name == "translate":
            t = (1, 0, 0, 1, v[0], v[1] if len(v) > 1 else 0)
        elif name == "scale":
            sx = v[0]
            sy = v[1] if len(v) > 1 else sx
            t = (sx, 0, 0, sy, 0, 0)
        elif name == "rotate":
            r = math.radians(v[0])
            cs, sn = math.cos(r), math.sin(r)
            t = (cs, sn, -sn, cs, 0, 0)
            if len(v) == 3:
                t = mmul(mmul((1, 0, 0, 1, v[1], v[2]), t), (1, 0, 0, 1, -v[1], -v[2]))
        elif name == "skewX":
            t = (1, 0, math.tan(math.radians(v[0])), 1, 0, 0)
        elif name == "skewY":
            t = (1, math.tan(math.radians(v[0])), 0, 1, 0, 0)
        m = mmul(m, t)
    return m


# ==========================================================================
#  Геометрия. Подпуть = {"start": (x,y), "segs": [("L",p) | ("C",c1,c2,p)], "closed": bool}
# ==========================================================================

def arc_to_cubics(p0, rx, ry, phi, large, sweep, p1):
    """Эллиптическая дуга SVG → кубические кривые (куски ≤ 90°)."""
    if p0 == p1:
        return []
    if rx == 0 or ry == 0:
        return [("L", p1)]
    rx, ry = abs(rx), abs(ry)
    cp, sp = math.cos(math.radians(phi)), math.sin(math.radians(phi))
    dx, dy = (p0[0] - p1[0]) / 2, (p0[1] - p1[1]) / 2
    x1 = cp * dx + sp * dy
    y1 = -sp * dx + cp * dy
    lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry)
    if lam > 1:
        s = math.sqrt(lam)
        rx, ry = rx * s, ry * s
    num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
    den = rx * rx * y1 * y1 + ry * ry * x1 * x1
    co = math.sqrt(max(0.0, num / den)) if den else 0.0
    if large == sweep:
        co = -co
    cxp, cyp = co * rx * y1 / ry, -co * ry * x1 / rx
    cx = cp * cxp - sp * cyp + (p0[0] + p1[0]) / 2
    cy = sp * cxp + cp * cyp + (p0[1] + p1[1]) / 2

    def ang(ux, uy, vx, vy):
        a = math.atan2(ux * vy - uy * vx, ux * vx + uy * vy)
        return a

    t1 = ang(1, 0, (x1 - cxp) / rx, (y1 - cyp) / ry)
    dt = ang((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry)
    if not sweep and dt > 0:
        dt -= 2 * math.pi
    elif sweep and dt < 0:
        dt += 2 * math.pi
    n = max(1, int(math.ceil(abs(dt) / (math.pi / 2) - 1e-9)))
    step = dt / n
    k = 4.0 / 3.0 * math.tan(step / 4)
    segs = []

    def pt(t):
        x, y = rx * math.cos(t), ry * math.sin(t)
        return (cp * x - sp * y + cx, sp * x + cp * y + cy)

    def der(t):
        x, y = -rx * math.sin(t), ry * math.cos(t)
        return (cp * x - sp * y, sp * x + cp * y)

    t = t1
    for i in range(n):
        a, b = t, t + step
        pa, pb = pt(a), pt(b)
        da, db = der(a), der(b)
        c1 = (pa[0] + k * da[0], pa[1] + k * da[1])
        c2 = (pb[0] - k * db[0], pb[1] - k * db[1])
        segs.append(("C", c1, c2, p1 if i == n - 1 else pb))
        t = b
    return segs


def parse_path(d):
    # посимвольный разбор: флаги дуг могут быть слитными ("a1 1 0 01 5 5")
    subs, cur = [], None
    x = y = 0.0
    last_c2 = None
    last_q = None
    cmd = None
    raw = d or ""
    pos = 0
    n = len(raw)
    ws = " \t\r\n,"

    def skip():
        nonlocal pos
        while pos < n and raw[pos] in ws:
            pos += 1

    def read_num():
        nonlocal pos
        skip()
        m = NUM_RE.match(raw, pos)
        if not m:
            raise ValueError("число ожидалось в позиции %d" % pos)
        pos = m.end()
        return float(m.group(0))

    def read_flag():
        nonlocal pos
        skip()
        ch = raw[pos]
        if ch not in "01":
            raise ValueError("флаг дуги ожидался в позиции %d" % pos)
        pos += 1
        return int(ch)

    def has_num():
        skip()
        return pos < n and (raw[pos].isdigit() or raw[pos] in "+-.")

    def new_sub(p):
        nonlocal cur
        cur = {"start": p, "segs": [], "closed": False}
        subs.append(cur)

    while True:
        skip()
        if pos >= n:
            break
        ch = raw[pos]
        if ch.isalpha():
            cmd = ch
            pos += 1
        elif cmd is None or cmd in "Zz":
            raise ValueError("число без команды в позиции %d" % pos)
        elif cmd in "Mm":
            cmd = "L" if cmd == "M" else "l"
        rel = cmd.islower()
        C = cmd.upper()
        if C == "Z":
            if cur is not None:
                cur["closed"] = True
                x, y = cur["start"]
                cur = None
            last_c2 = last_q = None
            continue
        if C == "M":
            px, py = read_num(), read_num()
            if rel:
                px, py = px + x, py + y
            x, y = px, py
            new_sub((x, y))
            last_c2 = last_q = None
            cmd = "l" if rel else "L"
            continue
        if cur is None:
            new_sub((x, y))
        if C == "L":
            px, py = read_num(), read_num()
            if rel:
                px, py = px + x, py + y
            x, y = px, py
            cur["segs"].append(("L", (x, y)))
            last_c2 = last_q = None
        elif C == "H":
            px = read_num()
            x = px + x if rel else px
            cur["segs"].append(("L", (x, y)))
            last_c2 = last_q = None
        elif C == "V":
            py = read_num()
            y = py + y if rel else py
            cur["segs"].append(("L", (x, y)))
            last_c2 = last_q = None
        elif C in "CS":
            if C == "C":
                v = [read_num() for _ in range(6)]
                c1 = (v[0], v[1])
            else:
                v = [0, 0] + [read_num() for _ in range(4)]
                c1 = (2 * x - last_c2[0], 2 * y - last_c2[1]) if last_c2 else (x, y)
            if rel:
                if C == "C":
                    c1 = (c1[0] + x, c1[1] + y)
                c2 = (v[2] + x, v[3] + y)
                p = (v[4] + x, v[5] + y)
            else:
                c2 = (v[2], v[3])
                p = (v[4], v[5])
            cur["segs"].append(("C", c1, c2, p))
            last_c2, last_q = c2, None
            x, y = p
        elif C in "QT":
            if C == "Q":
                v = [read_num() for _ in range(4)]
                q = (v[0] + x, v[1] + y) if rel else (v[0], v[1])
                p = (v[2] + x, v[3] + y) if rel else (v[2], v[3])
            else:
                v = [read_num() for _ in range(2)]
                q = (2 * x - last_q[0], 2 * y - last_q[1]) if last_q else (x, y)
                p = (v[0] + x, v[1] + y) if rel else (v[0], v[1])
            c1 = (x + 2.0 / 3 * (q[0] - x), y + 2.0 / 3 * (q[1] - y))
            c2 = (p[0] + 2.0 / 3 * (q[0] - p[0]), p[1] + 2.0 / 3 * (q[1] - p[1]))
            cur["segs"].append(("C", c1, c2, p))
            last_q, last_c2 = q, None
            x, y = p
        elif C == "A":
            rx, ry, phi = read_num(), read_num(), read_num()
            large, sweep = read_flag(), read_flag()
            px, py = read_num(), read_num()
            if rel:
                px, py = px + x, py + y
            cur["segs"].extend(arc_to_cubics((x, y), rx, ry, phi, large, sweep, (px, py)))
            x, y = px, py
            last_c2 = last_q = None
        else:
            raise ValueError("неизвестная команда пути: " + cmd)
    return [s for s in subs if s["segs"]]


def poly(points, closed):
    if len(points) < 2:
        return []
    return [{"start": points[0], "segs": [("L", p) for p in points[1:]], "closed": closed}]


def rect_path(x, y, w, h, rx, ry):
    if w <= 0 or h <= 0:
        return []
    if rx <= 0 and ry <= 0:
        return poly([(x, y), (x + w, y), (x + w, y + h), (x, y + h)], True)
    if rx <= 0:
        rx = ry
    if ry <= 0:
        ry = rx
    rx, ry = min(rx, w / 2), min(ry, h / 2)
    k = 0.5522847498
    s = {"start": (x + rx, y), "segs": [], "closed": True}
    S = s["segs"]
    S.append(("L", (x + w - rx, y)))
    S.append(("C", (x + w - rx + k * rx, y), (x + w, y + ry - k * ry), (x + w, y + ry)))
    S.append(("L", (x + w, y + h - ry)))
    S.append(("C", (x + w, y + h - ry + k * ry), (x + w - rx + k * rx, y + h), (x + w - rx, y + h)))
    S.append(("L", (x + rx, y + h)))
    S.append(("C", (x + rx - k * rx, y + h), (x, y + h - ry + k * ry), (x, y + h - ry)))
    S.append(("L", (x, y + ry)))
    S.append(("C", (x, y + ry - k * ry), (x + rx - k * rx, y), (x + rx, y)))
    return [s]


def ellipse_path(cx, cy, rx, ry):
    if rx <= 0 or ry <= 0:
        return []
    k = 0.5522847498
    return [{"start": (cx + rx, cy), "closed": True, "segs": [
        ("C", (cx + rx, cy + k * ry), (cx + k * rx, cy + ry), (cx, cy + ry)),
        ("C", (cx - k * rx, cy + ry), (cx - rx, cy + k * ry), (cx - rx, cy)),
        ("C", (cx - rx, cy - k * ry), (cx - k * rx, cy - ry), (cx, cy - ry)),
        ("C", (cx + k * rx, cy - ry), (cx + rx, cy - k * ry), (cx + rx, cy)),
    ]}]


def transform_subs(subs, m):
    out = []
    for s in subs:
        segs = []
        for seg in s["segs"]:
            if seg[0] == "L":
                segs.append(("L", mapply(m, seg[1])))
            else:
                segs.append(("C", mapply(m, seg[1]), mapply(m, seg[2]), mapply(m, seg[3])))
        out.append({"start": mapply(m, s["start"]), "segs": segs, "closed": s["closed"]})
    return out


def cubic_extrema(p0, p1, p2, p3):
    ts = []
    for i in (0, 1):
        a = -p0[i] + 3 * p1[i] - 3 * p2[i] + p3[i]
        b = 2 * (p0[i] - 2 * p1[i] + p2[i])
        c = p1[i] - p0[i]
        if abs(a) < 1e-12:
            if abs(b) > 1e-12:
                ts.append(-c / b)
        else:
            disc = b * b - 4 * a * c
            if disc >= 0:
                r = math.sqrt(disc)
                ts += [(-b + r) / (2 * a), (-b - r) / (2 * a)]
    pts = [p0, p3]
    for t in ts:
        if 0 < t < 1:
            mt = 1 - t
            pts.append(tuple(mt ** 3 * p0[i] + 3 * mt * mt * t * p1[i] + 3 * mt * t * t * p2[i] + t ** 3 * p3[i]
                             for i in (0, 1)))
    return pts


def bbox(subs):
    pts = []
    for s in subs:
        cur = s["start"]
        pts.append(cur)
        for seg in s["segs"]:
            if seg[0] == "L":
                pts.append(seg[1])
                cur = seg[1]
            else:
                pts += cubic_extrema(cur, seg[1], seg[2], seg[3])
                cur = seg[3]
    if not pts:
        return (0, 0, 0, 0)
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return (min(xs), min(ys), max(xs) - min(xs), max(ys) - min(ys))


def flatten(s, steps=8):
    pts = [s["start"]]
    cur = s["start"]
    for seg in s["segs"]:
        if seg[0] == "L":
            pts.append(seg[1])
            cur = seg[1]
        else:
            p0, p1, p2, p3 = cur, seg[1], seg[2], seg[3]
            for i in range(1, steps + 1):
                t = i / steps
                mt = 1 - t
                pts.append(tuple(mt ** 3 * p0[k] + 3 * mt * mt * t * p1[k] + 3 * mt * t * t * p2[k] + t ** 3 * p3[k]
                                 for k in (0, 1)))
            cur = p3
    return pts


def signed_area(pts):
    return sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1]
               for i in range(len(pts))) / 2


def point_in_poly(p, pts):
    inside = False
    j = len(pts) - 1
    for i in range(len(pts)):
        xi, yi = pts[i]
        xj, yj = pts[j]
        if (yi > p[1]) != (yj > p[1]) and p[0] < (xj - xi) * (p[1] - yi) / ((yj - yi) or 1e-12) + xi:
            inside = not inside
        j = i
    return inside


def reverse_sub(s):
    pts = [s["start"]]
    for seg in s["segs"]:
        pts.append(seg[-1])
    segs = []
    for i in range(len(s["segs"]) - 1, -1, -1):
        seg = s["segs"][i]
        prev = pts[i]
        if seg[0] == "L":
            segs.append(("L", prev))
        else:
            segs.append(("C", seg[2], seg[1], prev))
    return {"start": pts[-1], "segs": segs, "closed": s["closed"]}


def evenodd_to_nonzero(subs):
    """HVIF заливает только по правилу nonzero. Для evenodd меняем направление
    вложенных контуров так, чтобы чётность вложенности давала то же изображение.
    Работает для непересекающихся контуров (обычный случай: дырки, кольца)."""
    if len(subs) < 2:
        return subs
    flats = [flatten(s) for s in subs]
    out = []
    for i, s in enumerate(subs):
        f = flats[i]
        probe = f[0]
        depth = sum(1 for j, g in enumerate(flats) if j != i and len(g) > 2 and point_in_poly(probe, g))
        a = signed_area(f)
        want_positive = (depth % 2 == 0)
        if (a > 0) != want_positive and abs(a) > 1e-9:
            s = reverse_sub(s)
        out.append(s)
    return out


# ==========================================================================
#  Цвета и стили
# ==========================================================================
NAMED = {
    "black": (0, 0, 0), "white": (255, 255, 255), "red": (255, 0, 0), "green": (0, 128, 0),
    "blue": (0, 0, 255), "yellow": (255, 255, 0), "cyan": (0, 255, 255), "magenta": (255, 0, 255),
    "gray": (128, 128, 128), "grey": (128, 128, 128), "orange": (255, 165, 0), "purple": (128, 0, 128),
    "silver": (192, 192, 192), "lime": (0, 255, 0), "navy": (0, 0, 128), "teal": (0, 128, 128),
    "maroon": (128, 0, 0), "olive": (128, 128, 0), "aqua": (0, 255, 255), "fuchsia": (255, 0, 255),
}


def parse_color(s, current=None):
    """→ (r, g, b, a 0..1) или None для none/transparent."""
    if s is None:
        return None
    s = s.strip()
    low = s.lower()
    if low in ("none", "transparent", ""):
        return None
    if low == "currentcolor":
        return parse_color(current or "#000000")
    if s.startswith("#"):
        h = s[1:]
        if len(h) in (3, 4):
            h = "".join(c * 2 for c in h)
        r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
        a = int(h[6:8], 16) / 255 if len(h) == 8 else 1.0
        return (r, g, b, a)
    m = re.match(r"rgba?\(([^)]*)\)", low)
    if m:
        parts = [p.strip() for p in m.group(1).replace("/", ",").split(",") if p.strip()]
        vals = []
        for i, p in enumerate(parts[:3]):
            vals.append(round(float(p[:-1]) * 2.55) if p.endswith("%") else round(float(p)))
        a = 1.0
        if len(parts) > 3:
            a = float(parts[3][:-1]) / 100 if parts[3].endswith("%") else float(parts[3])
        return (vals[0], vals[1], vals[2], a)
    if low in NAMED:
        return NAMED[low] + (1.0,)
    raise ValueError("не понимаю цвет: " + s)


INHERITED = {"fill", "stroke", "stroke-width", "stroke-linejoin", "stroke-linecap", "stroke-miterlimit",
             "fill-opacity", "stroke-opacity", "fill-rule", "color", "font-family", "font-size",
             "font-weight", "text-anchor", "letter-spacing", "visibility", "shape-rendering",
             "stroke-dasharray"}
NONINHERITED = {"opacity", "filter", "clip-path", "mask", "display"}


def local_style(el):
    st = {}
    for k in INHERITED | NONINHERITED:
        if k in el.attrib:
            st[k] = el.attrib[k]
    for decl in (el.attrib.get("style") or "").split(";"):
        if ":" in decl:
            k, v = decl.split(":", 1)
            k = k.strip()
            if k in INHERITED or k in NONINHERITED:
                st[k] = v.strip()
    return st


def tag(el):
    return el.tag.split("}")[-1]


def href(el):
    for k, v in el.attrib.items():
        if k == "href" or k.endswith("}href"):
            return v
    return None


def length(v, ref=0.0, default=0.0):
    if v is None or v == "":
        return default
    v = v.strip()
    if v.endswith("%"):
        return float(v[:-1]) / 100 * ref
    m = NUM_RE.match(v)
    return float(m.group(0)) if m else default


# ==========================================================================
#  Текст → контуры (fontTools)
# ==========================================================================
class FontBook:
    def __init__(self, dirs, warn):
        self.dirs = [d for d in dirs if d and os.path.isdir(d)]
        self.cache = {}
        self.warn = warn
        import importlib.util
        self.ok = importlib.util.find_spec("fontTools") is not None

    def _find(self, family):
        key = re.sub(r"[^a-z0-9]", "", family.lower())
        for d in self.dirs:
            for root, _, files in os.walk(d):
                for f in sorted(files):
                    if f.lower().endswith((".ttf", ".otf")):
                        if re.sub(r"[^a-z0-9]", "", f.lower()).startswith(key):
                            return os.path.join(root, f)
        return None

    def get(self, families, weight):
        from fontTools.ttLib import TTFont
        for fam in [f.strip().strip("'\"") for f in families.split(",")]:
            if fam.lower() in ("sans-serif", "serif", "monospace", "arial", "helvetica"):
                continue
            key = (fam, weight)
            if key in self.cache:
                return self.cache[key]
            path = self._find(fam)
            if not path:
                continue
            font = TTFont(path)
            if "fvar" in font:
                from fontTools.varLib import instancer
                axes = {a.axisTag: a for a in font["fvar"].axes}
                loc = {}
                if "wght" in axes:
                    a = axes["wght"]
                    loc["wght"] = max(a.minValue, min(a.maxValue, weight))
                for t, a in axes.items():
                    loc.setdefault(t, a.defaultValue)
                font = instancer.instantiateVariableFont(font, loc)
            self.cache[key] = font
            return font
        return None


def text_runs(el, st, fonts, warn):
    """<text> с вложенными <tspan> → [(подпути, стиль)]. Без кернинга и лигатур."""
    runs = []

    def collect(node, nst):
        if node.text:
            runs.append([node.text, nst])
        for ch in node:
            if tag(ch) == "tspan":
                cst = dict(nst)
                cst.update(local_style(ch))
                collect(ch, cst)
            if ch.tail:
                runs.append([ch.tail, nst])

    collect(el, st)
    runs = [r for r in runs if r[0].strip() or len(runs) > 1]
    if not runs:
        return []
    runs[0][0] = runs[0][0].lstrip()
    runs[-1][0] = runs[-1][0].rstrip()
    content = "".join(r[0] for r in runs)
    if not content:
        return []
    if not fonts.ok:
        warn("текст «%s» пропущен: нет fontTools (pip install fonttools)" % content)
        return []
    from fontTools.pens.basePen import BasePen
    layout = []  # (font, glyph, x, scale, style)
    adv = 0.0
    last_ls = 0.0
    for text, rst in runs:
        fam = rst.get("font-family", "sans-serif")
        w = rst.get("font-weight", "400")
        weight = {"normal": 400, "bold": 700}.get(w, None) or float(w)
        font = fonts.get(fam, weight)
        if font is None:
            warn("текст «%s» пропущен: шрифт «%s» не найден в --fonts" % (content, fam))
            return []
        sc = length(rst.get("font-size"), default=16) / font["head"].unitsPerEm
        cmap = font.getBestCmap()
        hmtx = font["hmtx"]
        ls = length(rst.get("letter-spacing"), default=0) if rst.get("letter-spacing", "normal") != "normal" else 0
        for ch in text:
            g = cmap.get(ord(ch))
            if g is None:
                warn("в шрифте нет символа «%s»" % ch)
                g = ".notdef"
            layout.append((font, g, adv, sc, rst))
            adv += hmtx[g][0] * sc + ls
            last_ls = ls
    width = adv - last_ls
    x0 = (nums(el.attrib.get("x", "0"))[:1] or [0.0])[0]
    y0 = (nums(el.attrib.get("y", "0"))[:1] or [0.0])[0]
    anchor = st.get("text-anchor", "start")
    if anchor == "middle":
        x0 -= width / 2
    elif anchor == "end":
        x0 -= width

    class Pen(BasePen):
        def __init__(self, gs, ox, sc, out):
            BasePen.__init__(self, gs)
            self.ox, self.sc, self.out, self.cur = ox, sc, out, None

        def T(self, p):
            return (self.ox + p[0] * self.sc, y0 - p[1] * self.sc)

        def _moveTo(self, p):
            self.cur = {"start": self.T(p), "segs": [], "closed": False}
            self.out.append(self.cur)

        def _lineTo(self, p):
            self.cur["segs"].append(("L", self.T(p)))

        def _curveToOne(self, a, b, c):
            self.cur["segs"].append(("C", self.T(a), self.T(b), self.T(c)))

        def _qCurveToOne(self, q, p):
            p0 = self._getCurrentPoint()
            c1 = (p0[0] + 2 / 3 * (q[0] - p0[0]), p0[1] + 2 / 3 * (q[1] - p0[1]))
            c2 = (p[0] + 2 / 3 * (q[0] - p[0]), p[1] + 2 / 3 * (q[1] - p[1]))
            self.cur["segs"].append(("C", self.T(c1), self.T(c2), self.T(p)))

        def _closePath(self):
            if self.cur:
                self.cur["closed"] = True

        def _endPath(self):
            pass

    out = []
    for font, g, ox, sc, rst in layout:
        subs = []
        gs = font.getGlyphSet()
        gs[g].draw(Pen(gs, x0 + ox, sc, subs))
        subs = [s for s in subs if s["segs"]]
        if not subs:
            continue
        if out and out[-1][1] is rst:
            out[-1][0].extend(subs)
        else:
            out.append((subs, rst))
    return out


# ==========================================================================
#  Обход SVG → список элементов для отрисовки
# ==========================================================================
class Item:
    __slots__ = ("subs", "ctm", "st", "opacity", "shadow", "hint")

    def __init__(self, subs, ctm, st, opacity, hint):
        self.subs, self.ctm, self.st, self.opacity, self.hint = subs, ctm, st, opacity, hint
        self.shadow = None


class Reader:
    def __init__(self, root, opts, warn):
        self.root = root
        self.opts = opts
        self.warn = warn
        self.ids = {}
        for el in root.iter():
            if "id" in el.attrib:
                self.ids[el.attrib["id"]] = el
        self.fonts = FontBook(opts.fonts, warn)
        self.warned = set()

    def warn_once(self, key, msg):
        if key not in self.warned:
            self.warned.add(key)
            self.warn(msg)

    def walk(self, el, ctm, inh, opacity, depth=0):
        t = tag(el)
        if t in ("defs", "metadata", "title", "desc", "style", "script", "linearGradient", "radialGradient",
                 "filter", "clipPath", "mask", "pattern", "symbol", "marker"):
            return []
        st = dict((k, v) for k, v in inh.items() if k in INHERITED)
        st.update(local_style(el))
        if st.get("display") == "none":
            return []
        if "transform" in el.attrib:
            ctm = mmul(ctm, parse_transform(el.attrib["transform"]))
        op = opacity * float(st.get("opacity", "1") or 1)
        for k in ("clip-path", "mask"):
            if st.get(k, "none") != "none":
                self.warn_once(k, "%s не поддерживается в HVIF — проигнорирован (вырежьте геометрию в редакторе)" % k)
        items = []
        if t in ("svg", "g", "a", "switch"):
            for ch in el:
                items += self.walk(ch, ctm, st, op, depth + 1)
        elif t == "use":
            ref = href(el)
            target = self.ids.get((ref or "")[1:])
            if target is None:
                self.warn("use: не найден %s" % ref)
            else:
                m = mmul(ctm, (1, 0, 0, 1, length(el.attrib.get("x")), length(el.attrib.get("y"))))
                if tag(target) == "symbol":
                    for ch in target:
                        items += self.walk(ch, m, st, op, depth + 1)
                else:
                    items += self.walk(target, m, st, op, depth + 1)
        elif t == "text":
            if st.get("visibility", "visible") not in ("hidden", "collapse"):
                for subs, rst in text_runs(el, st, self.fonts, self.warn):
                    items.append(Item(subs, ctm, rst, op, False))
        else:
            subs = self.geometry(el, t, st)
            if subs:
                if st.get("visibility", "visible") in ("hidden", "collapse"):
                    return []
                hint = st.get("shape-rendering") == "crispEdges"
                items.append(Item(subs, ctm, st, op, hint))
        flt = st.get("filter", "none")
        if flt != "none" and items:
            items = self.apply_filter(flt, items)
        return items

    def geometry(self, el, t, st):
        a = el.attrib
        if t == "path":
            try:
                return parse_path(a.get("d", ""))
            except Exception as e:
                self.warn("ошибка в пути: %s" % e)
                return []
        if t == "rect":
            return rect_path(length(a.get("x")), length(a.get("y")), length(a.get("width")),
                             length(a.get("height")), length(a.get("rx")), length(a.get("ry")))
        if t == "circle":
            r = length(a.get("r"))
            return ellipse_path(length(a.get("cx")), length(a.get("cy")), r, r)
        if t == "ellipse":
            return ellipse_path(length(a.get("cx")), length(a.get("cy")), length(a.get("rx")), length(a.get("ry")))
        if t == "line":
            return poly([(length(a.get("x1")), length(a.get("y1"))), (length(a.get("x2")), length(a.get("y2")))],
                        False)
        if t in ("polyline", "polygon"):
            v = nums(a.get("points"))
            return poly(list(zip(v[0::2], v[1::2])), t == "polygon")
        if t in ("image", "foreignObject"):
            self.warn_once(t, "<%s> не поддерживается в HVIF — пропущен" % t)
        return []

    def apply_filter(self, ref, items):
        m = re.match(r"url\(\s*#([^)\s]+)\s*\)", ref)
        flt = self.ids.get(m.group(1)) if m else None
        if flt is None:
            return items
        drop = None
        blur_only = False
        for p in flt.iter():
            pt = tag(p)
            if pt == "feDropShadow":
                drop = p
            elif pt == "feGaussianBlur":
                blur_only = True
        if drop is not None and self.opts.shadows:
            col = parse_color(local_style(drop).get("flood-color", drop.attrib.get("flood-color", "#000"))) \
                or (0, 0, 0, 1)
            fo = float(drop.attrib.get("flood-opacity", local_style(drop).get("flood-opacity", "1")))
            dx, dy = length(drop.attrib.get("dx"), default=2), length(drop.attrib.get("dy"), default=2)
            sd = nums(drop.attrib.get("stdDeviation", "2"))[0]
            shadows = []
            for it in items:
                sh = Item(it.subs, mmul(it.ctm, (1, 0, 0, 1, dx, dy)), it.st, it.opacity, False)
                sh.shadow = (col, fo, sd)
                shadows.append(sh)
            self.warn_once("shadow", "feDropShadow заменена приближением без размытия (2 слоя); "
                                     "--no-shadows отключает")
            return shadows + items
        if drop is not None:
            self.warn_once("shadow-off", "feDropShadow пропущена (--no-shadows)")
        elif blur_only:
            self.warn_once("blur", "feGaussianBlur (свечение) не поддерживается в HVIF — пропущен")
        return items


# ==========================================================================
#  Кодирование HVIF
# ==========================================================================
def coord_q(v):
    """Значение, которое получит декодер после write_coord/read_coord."""
    v = max(-128.0, min(192.0, v))
    r = round(v)
    if abs(v - r) < 1e-4 and -32 <= r <= 95:
        return float(r)
    return round((v + 128.0) * 102.0) / 102.0 - 128.0


def coord_bytes(v):
    v = max(-128.0, min(192.0, v))
    r = round(v)
    if abs(v - r) < 1e-4 and -32 <= r <= 95:
        return bytes([int(r) + 32])
    iv = int(round((v + 128.0) * 102.0))
    iv = min(iv, 32767) | 0x8000
    return bytes([iv >> 8, iv & 0xFF])


def float24(v):
    bits = struct.unpack(">I", struct.pack(">f", v))[0]
    sign = (bits >> 31) & 1
    exp = ((bits >> 23) & 0xFF) - 127
    man = bits & 0x7FFFFF
    if v == 0 or exp >= 32 or exp < -32:
        return b"\0\0\0"
    sv = (sign << 23) | ((exp + 32) << 17) | (man >> 6)
    return bytes([(sv >> 16) & 0xFF, (sv >> 8) & 0xFF, sv & 0xFF])


def float24_decode(b):
    sv = (b[0] << 16) | (b[1] << 8) | b[2]
    if sv == 0:
        return 0.0
    sign = (sv & 0x800000) >> 23
    exp = ((sv & 0x7E0000) >> 17) - 32
    man = (sv & 0x01FFFF) << 6
    return struct.unpack(">f", struct.pack(">I", (sign << 31) | ((exp + 127) << 23) | man))[0]


def snap(v, eps=0.004):
    r = round(v)
    return float(r) if abs(v - r) < eps else v


def subs_to_points(subs, xf):
    """Подпути → точки HVIF [(p, in, out)] с замкнутостью. xf — преобразование координат."""
    paths = []
    for s in subs:
        P = lambda p: tuple(coord_q(snap(c)) for c in mapply(xf, p))  # noqa: E731
        st = P(s["start"])
        pts = [[st, st, st]]
        for seg in s["segs"]:
            if seg[0] == "L":
                p = P(seg[1])
                if p == pts[-1][0] and pts[-1][2] == p:
                    continue
                pts.append([p, p, p])
            else:
                c1, c2, p = P(seg[1]), P(seg[2]), P(seg[3])
                pts[-1][2] = c1
                pts.append([p, c2, p])
        closed = s["closed"]
        if closed and len(pts) > 2 and pts[-1][0] == pts[0][0]:
            # замкнутый контур вернулся в начало: входящая ручка переходит на первую точку
            pts[0][1] = pts[-1][1]
            pts.pop()
        if len(pts) < 2:
            continue
        paths.append((pts, closed))
    return paths


def encode_path(pts, closed):
    flags = PF_CLOSED if closed else 0
    n = len(pts)
    no_curves = all(p[1] == p[0] and p[2] == p[0] for p in pts)
    cands = []
    if no_curves:
        data = b"".join(coord_bytes(p[0][0]) + coord_bytes(p[0][1]) for p in pts)
        cands.append((flags | PF_NO_CURVES, data))
    # команды
    cmds, data = [], b""
    last = (0.0, 0.0)
    for p, pin, pout in pts:
        if pin == p and pout == p:
            if p[1] == last[1]:
                cmds.append(CMD_H)
                data += coord_bytes(p[0])
            elif p[0] == last[0]:
                cmds.append(CMD_V)
                data += coord_bytes(p[1])
            else:
                cmds.append(CMD_L)
                data += coord_bytes(p[0]) + coord_bytes(p[1])
        else:
            cmds.append(CMD_C)
            data += b"".join(coord_bytes(c) for c in (p[0], p[1], pin[0], pin[1], pout[0], pout[1]))
        last = p
    cb = bytearray((n + 3) // 4)
    for i, c in enumerate(cmds):
        cb[i // 4] |= c << (2 * (i % 4))
    cands.append((flags | PF_COMMANDS, bytes(cb) + data))
    full = b"".join(coord_bytes(c) for p, pin, pout in pts for c in (p[0], p[1], pin[0], pin[1], pout[0], pout[1]))
    cands.append((flags, full))
    f, d = min(cands, key=lambda x: len(x[1]))
    return bytes([f, n]) + d


def color_bytes(c, opacity):
    r, g, b, a = c
    A = max(0, min(255, int(round(a * opacity * 255))))
    return (int(r), int(g), int(b), A)


class Builder:
    def __init__(self, warn, current_color, hinting):
        self.styles, self.paths, self.shapes = [], [], []
        self.warn = warn
        self.current = current_color
        self.hinting = hinting

    def _idx(self, lst, blob):
        if blob in lst:
            return lst.index(blob)
        lst.append(blob)
        return len(lst) - 1

    # --- стили ---
    def solid_style(self, rgba):
        r, g, b, a = rgba
        if r == g == b:
            blob = bytes([STYLE_GRAY_NO_ALPHA, r]) if a == 255 else bytes([STYLE_GRAY, r, a])
        else:
            blob = bytes([STYLE_SOLID_NO_ALPHA, r, g, b]) if a == 255 else bytes([STYLE_SOLID, r, g, b, a])
        return self._idx(self.styles, blob)

    def gradient_style(self, gtype, matrix, stops):
        alpha = any(c[3] != 255 for _, c in stops)
        gray = all(c[0] == c[1] == c[2] for _, c in stops)
        flags = GF_TRANSFORM | (0 if alpha else GF_NO_ALPHA) | (GF_GRAYS if gray else 0)
        blob = bytes([STYLE_GRADIENT, gtype, flags, len(stops)]) + b"".join(float24(v) for v in matrix)
        for off, c in stops:
            blob += bytes([max(0, min(255, int(off * 255.0)))])
            if alpha:
                blob += bytes([c[0], c[3]]) if gray else bytes(c)
            else:
                blob += bytes([c[0]]) if gray else bytes(c[:3])
        return self._idx(self.styles, blob)

    # --- фигуры ---
    def add_shape(self, style_idx, path_blobs, matrix, transformers, hint):
        idx = [self._idx(self.paths, p) for p in path_blobs]
        flags = 0
        extra = b""
        if matrix is not None and matrix != IDENT:
            if abs(matrix[0] - 1) < 1e-9 and abs(matrix[3] - 1) < 1e-9 and abs(matrix[1]) < 1e-9 \
                    and abs(matrix[2]) < 1e-9 and abs(coord_q(matrix[4]) - matrix[4]) < 1e-6 \
                    and abs(coord_q(matrix[5]) - matrix[5]) < 1e-6:
                flags |= SF_TRANSLATION
                extra = coord_bytes(matrix[4]) + coord_bytes(matrix[5])
            else:
                flags |= SF_TRANSFORM
                extra = b"".join(float24(v) for v in matrix)
        if hint:
            flags |= SF_HINTING
        tr = b""
        if transformers:
            flags |= SF_TRANSFORMERS
            tr = bytes([len(transformers)]) + b"".join(transformers)
        self.shapes.append(bytes([SHAPE_PATH_SOURCE, style_idx, len(idx)] + idx + [flags]) + extra + tr)

    def build(self):
        for name, lst in (("стилей", self.styles), ("путей", self.paths), ("фигур", self.shapes)):
            if len(lst) > 255:
                raise ValueError("слишком много %s: %d (максимум 255)" % (name, len(lst)))
        return MAGIC + bytes([len(self.styles)]) + b"".join(self.styles) + \
            bytes([len(self.paths)]) + b"".join(self.paths) + \
            bytes([len(self.shapes)]) + b"".join(self.shapes)


def resolve_gradient(reader, gid):
    """Собирает градиент с учётом наследования через href."""
    chain = []
    el = reader.ids.get(gid)
    seen = set()
    while el is not None and id(el) not in seen:
        seen.add(id(el))
        chain.append(el)
        h = href(el)
        el = reader.ids.get(h[1:]) if h and h.startswith("#") else None
    if not chain:
        return None
    g = {"type": tag(chain[0])}
    for el in reversed(chain):
        for k, v in el.attrib.items():
            g[k.split("}")[-1]] = v
    stops = []
    for el in chain:
        ss = [s for s in el if tag(s) == "stop"]
        if ss:
            stops = ss
            break
    out = []
    last = 0.0
    for s in stops:
        sst = dict(s.attrib)
        for decl in (s.attrib.get("style") or "").split(";"):
            if ":" in decl:
                k, v = decl.split(":", 1)
                sst[k.strip()] = v.strip()
        off = sst.get("offset", "0")
        o = float(off[:-1]) / 100 if off.endswith("%") else float(off)
        o = max(last, min(1.0, max(0.0, o)))
        last = o
        c = parse_color(sst.get("stop-color", "#000")) or (0, 0, 0, 0)
        a = c[3] * float(sst.get("stop-opacity", "1"))
        out.append((o, (c[0], c[1], c[2], a)))
    g["stops"] = out
    return g


def paint_style(builder, reader, paint, item, canvas, opacity, subs_local, path_xf_inv):
    """Возвращает индекс стиля или None. path_xf_inv: icon → пространство пути (для градиента)."""
    m = re.match(r"url\(\s*#([^)\s]+)\s*\)", paint or "")
    if not m:
        c = parse_color(paint, item.st.get("color", builder.current))
        if c is None:
            return None
        return builder.solid_style(color_bytes(c, opacity))
    g = resolve_gradient(reader, m.group(1))
    if g is None or not g["stops"]:
        builder.warn("градиент #%s не найден" % m.group(1))
        return None
    stops = [(o, color_bytes(c, opacity)) for o, c in g["stops"]]
    if len(stops) == 1:
        return builder.solid_style(stops[0][1])
    if g.get("spreadMethod", "pad") != "pad":
        reader.warn_once("spread", "spreadMethod=%s не поддерживается — будет pad" % g["spreadMethod"])
    bbox_units = g.get("gradientUnits", "objectBoundingBox") != "userSpaceOnUse"
    if bbox_units:
        bx, by, bw, bh = bbox(subs_local)
        if bw == 0 or bh == 0:
            return builder.solid_style(stops[-1][1])
        U = (bw, 0, 0, bh, bx, by)
        rw = rh = 1.0
    else:
        U = IDENT
        rw, rh = reader.vb[2], reader.vb[3]
    GT = parse_transform(g.get("gradientTransform", ""))

    def L(k, ref, d):
        v = g.get(k)
        if v is None:
            return d
        v = v.strip()
        if v.endswith("%"):
            return float(v[:-1]) / 100 * (1.0 if bbox_units else ref)
        return float(v)

    if g["type"] == "linearGradient":
        x1, y1 = L("x1", rw, 0.0), L("y1", rh, 0.0)
        x2, y2 = L("x2", rw, 1.0 if bbox_units else rw), L("y2", rh, 0.0)
        if x1 == x2 and y1 == y2:
            return builder.solid_style(stops[-1][1])
        ux, uy = (x2 - x1) / 128.0, (y2 - y1) / 128.0
        B = (ux, uy, -uy, ux, (x1 + x2) / 2, (y1 + y2) / 2)
        gtype = GRAD_LINEAR
    else:
        cx, cy = L("cx", rw, 0.5 if bbox_units else rw / 2), L("cy", rh, 0.5 if bbox_units else rh / 2)
        r = L("r", math.hypot(rw, rh) / math.sqrt(2), 0.5 if bbox_units else math.hypot(rw, rh) / math.sqrt(2) / 2)
        if ("fx" in g and L("fx", rw, cx) != cx) or ("fy" in g and L("fy", rh, cy) != cy):
            reader.warn_once("focal", "фокус радиального градиента (fx/fy) не поддерживается — центр")
        if r <= 0:
            return builder.solid_style(stops[-1][1])
        B = (r / 64.0, 0, 0, r / 64.0, cx, cy)
        gtype = GRAD_CIRCULAR
    G = mmul(path_xf_inv, mmul(canvas, mmul(item.ctm, mmul(U, mmul(GT, B)))))
    return builder.gradient_style(gtype, G, stops)


def choose_k(width):
    """Масштаб пространства пути, чтобы целая ширина обводки HVIF была точной."""
    best = (abs(width - max(1, round(width))), 1)
    for k in (1, 2, 3, 4):
        w = width * k
        if round(w) >= 1:
            err = abs(w - round(w)) / k
            if err < 0.02:
                return k, 0.0
            best = min(best, (err, k))
    return best[1], best[0]


def convert(svg_text, opts, warn):
    root = ET.fromstring(svg_text)
    if tag(root) != "svg":
        raise ValueError("корневой элемент не <svg>")
    vb = nums(root.attrib.get("viewBox"))
    if len(vb) != 4:
        vb = [0, 0, length(root.attrib.get("width"), default=64), length(root.attrib.get("height"), default=64)]
    reader = Reader(root, opts, warn)
    reader.vb = vb
    s = CANVAS / max(vb[2], vb[3])
    ox = (CANVAS - vb[2] * s) / 2 - vb[0] * s
    oy = (CANVAS - vb[3] * s) / 2 - vb[1] * s
    canvas = (s, 0, 0, s, ox, oy)
    if vb[2] != vb[3]:
        warn("viewBox не квадратный (%gx%g) — вписан по центру в 64×64" % (vb[2], vb[3]))
    items = reader.walk(root, IDENT, {"fill": "#000000"}, 1.0)
    hint_all = opts.hinting == "on"
    b = Builder(warn, opts.current_color, opts.hinting)
    for it in items:
        st = it.st
        M = mmul(canvas, it.ctm)
        hint = hint_all or (opts.hinting == "auto" and it.hint)
        sc = mscale(M)
        a = math.hypot(M[0], M[1])
        c = math.hypot(M[2], M[3])
        if it.shadow:
            col, fo, sd = it.shadow
            base = (col[0], col[1], col[2], col[3])
            # слой 1 — мягкий край (обводка шириной ~2σ), слой 2 — ядро тени
            w = max(1.0, 2 * sd * sc)
            k, _ = choose_k(w)
            S = (1.0 / k, 0, 0, 1.0 / k, 32.0, 32.0) if k > 1 else IDENT
            xf = mmul(minv(S), M)
            pb = [encode_path(p, True) for p, _ in subs_to_points(it.subs, xf)]
            if pb:
                si = b.solid_style(color_bytes(base, fo * 0.22 * it.opacity))
                b.add_shape(si, pb, S if k > 1 else None, [bytes([TR_STROKE, int(round(w * k)) + 128, 2, 4])], False)
                pb1 = [encode_path(p, True) for p, _ in subs_to_points(it.subs, M)]
                si = b.solid_style(color_bytes(base, fo * 0.4 * it.opacity))
                b.add_shape(si, pb1, None, [], False)
            continue
        fill = st.get("fill", "#000000")
        subs_fill = it.subs
        if st.get("fill-rule") == "evenodd" and len(it.subs) > 1:
            subs_fill = evenodd_to_nonzero(it.subs)
        if fill and fill != "none":
            pts = subs_to_points(subs_fill, M)
            if any(len(p) > 255 for p, _ in pts):
                raise ValueError("в пути больше 255 точек — упростите контур")
            pb = [encode_path(p, cl) for p, cl in pts]
            if pb:
                op = it.opacity * float(st.get("fill-opacity", "1"))
                si = paint_style(b, reader, fill, it, canvas, op, it.subs, IDENT)
                if si is not None:
                    b.add_shape(si, pb, None, [], hint)
        stroke = st.get("stroke", "none")
        sw = length(st.get("stroke-width"), default=1.0)
        if stroke and stroke != "none" and sw > 0:
            if st.get("stroke-dasharray", "none") not in ("none", ""):
                reader.warn_once("dash", "stroke-dasharray не поддерживается — линия будет сплошной")
            if abs(a - c) > 1e-3 * max(a, c):
                reader.warn_once("aniso", "неравномерный масштаб: толщина обводки усреднена")
            w = sw * sc
            k, err = choose_k(w)
            if err > 0.05 * w:
                warn("обводка %.3g ед. округлена до %.3g (HVIF хранит целую ширину)" % (w, round(w * k) / k))
            if w * k > 127:
                raise ValueError("обводка слишком толстая")
            S = (1.0 / k, 0, 0, 1.0 / k, 32.0, 32.0) if k > 1 else IDENT
            xf = mmul(minv(S), M)
            pts = subs_to_points(it.subs, xf)
            if any(max(abs(c) for pt in p for q in pt for c in q) > 127.9 for p, _ in pts):
                warn("координаты вышли за пределы при масштабе ×%d — обводка округлена" % k)
                k, S, xf = 1, IDENT, M
                pts = subs_to_points(it.subs, xf)
            pb = [encode_path(p, cl) for p, cl in pts]
            if pb:
                op = it.opacity * float(st.get("stroke-opacity", "1"))
                si = paint_style(b, reader, stroke, it, canvas, op, it.subs, minv(S))
                if si is not None:
                    join = JOINS.get(st.get("stroke-linejoin", "miter"), 0)
                    cap = CAPS.get(st.get("stroke-linecap", "butt"), 0)
                    ml = int(round(float(st.get("stroke-miterlimit", "4"))))
                    tr = bytes([TR_STROKE, max(1, int(round(w * k))) + 128, join | (cap << 4), max(1, min(255, ml))])
                    b.add_shape(si, pb, S if k > 1 else None, [tr], hint)
    return b.build(), b


# ==========================================================================
#  Декодер HVIF → SVG (для проверки и просмотра)
# ==========================================================================
def decode(data):
    pos = 0

    def u8():
        nonlocal pos
        v = data[pos]
        pos += 1
        return v

    def coord():
        v = u8()
        if v & 128:
            lo = u8()
            return (((v & 127) << 8) | lo) / 102.0 - 128.0
        return v - 32.0

    def f24():
        nonlocal pos
        v = float24_decode(data[pos:pos + 3])
        pos += 3
        return v

    if data[:4] != MAGIC:
        raise ValueError("это не HVIF (нет сигнатуры ncif)")
    pos = 4
    styles = []
    for _ in range(u8()):
        t = u8()
        if t == STYLE_SOLID:
            styles.append(("solid", (u8(), u8(), u8(), u8())))
        elif t == STYLE_SOLID_NO_ALPHA:
            styles.append(("solid", (u8(), u8(), u8(), 255)))
        elif t == STYLE_GRAY:
            g, a = u8(), u8()
            styles.append(("solid", (g, g, g, a)))
        elif t == STYLE_GRAY_NO_ALPHA:
            g = u8()
            styles.append(("solid", (g, g, g, 255)))
        elif t == STYLE_GRADIENT:
            gt, fl, n = u8(), u8(), u8()
            m = tuple(f24() for _ in range(6)) if fl & GF_TRANSFORM else IDENT
            alpha, gray = not (fl & GF_NO_ALPHA), bool(fl & GF_GRAYS)
            stops = []
            for _ in range(n):
                o = u8() / 255.0
                if alpha:
                    if gray:
                        g, a = u8(), u8()
                        c = (g, g, g, a)
                    else:
                        c = (u8(), u8(), u8(), u8())
                else:
                    if gray:
                        g = u8()
                        c = (g, g, g, 255)
                    else:
                        c = (u8(), u8(), u8(), 255)
                stops.append((o, c))
            styles.append(("gradient", gt, m, stops))
        else:
            ln = u8() | (u8() << 8)
            pos += ln
            styles.append(("solid", (0, 0, 0, 0)))
    paths = []
    for _ in range(u8()):
        fl, n = u8(), u8()
        pts = []
        if fl & PF_NO_CURVES:
            for _ in range(n):
                p = (coord(), coord())
                pts.append((p, p, p))
        elif fl & PF_COMMANDS:
            cb = data[pos:pos + (n + 3) // 4]
            pos += (n + 3) // 4
            last = (0.0, 0.0)
            for i in range(n):
                c = (cb[i // 4] >> (2 * (i % 4))) & 3
                if c == CMD_H:
                    p = (coord(), last[1])
                    pts.append((p, p, p))
                elif c == CMD_V:
                    p = (last[0], coord())
                    pts.append((p, p, p))
                elif c == CMD_L:
                    p = (coord(), coord())
                    pts.append((p, p, p))
                else:
                    p = (coord(), coord())
                    pi = (coord(), coord())
                    po = (coord(), coord())
                    pts.append((p, pi, po))
                last = p
        else:
            for _ in range(n):
                p = (coord(), coord())
                pi = (coord(), coord())
                po = (coord(), coord())
                pts.append((p, pi, po))
        paths.append((pts, bool(fl & PF_CLOSED)))
    shapes = []
    for _ in range(u8()):
        t = u8()
        if t != SHAPE_PATH_SOURCE:
            ln = u8() | (u8() << 8)
            pos += ln
            continue
        si, n = u8(), u8()
        pidx = [u8() for _ in range(n)]
        fl = u8()
        m = IDENT
        if fl & SF_TRANSFORM:
            m = tuple(f24() for _ in range(6))
        elif fl & SF_TRANSLATION:
            m = (1, 0, 0, 1, coord(), coord())
        lod = None
        if fl & SF_LOD:
            lod = (u8() / 63.75, u8() / 63.75)
        trs = []
        if fl & SF_TRANSFORMERS:
            for _ in range(u8()):
                tt = u8()
                if tt == TR_STROKE:
                    w, lo, ml = u8(), u8(), u8()
                    trs.append(("stroke", w - 128.0, lo & 15, lo >> 4, ml))
                elif tt == TR_CONTOUR:
                    w, j, ml = u8(), u8(), u8()
                    trs.append(("contour", w - 128.0, j, ml))
                elif tt == TR_AFFINE:
                    v = struct.unpack("<6f", data[pos:pos + 24])
                    pos += 24
                    trs.append(("affine", v))
                elif tt == TR_PERSPECTIVE:
                    trs.append(("perspective", [f24() for _ in range(9)]))
                else:
                    ln = u8() | (u8() << 8)
                    pos += ln
        shapes.append({"style": si, "paths": pidx, "matrix": m, "hint": bool(fl & SF_HINTING), "lod": lod,
                       "transformers": trs})
    if pos != len(data):
        raise ValueError("лишние байты в конце: разобрано %d из %d" % (pos, len(data)))
    return {"styles": styles, "paths": paths, "shapes": shapes}


def fmt(v):
    s = ("%.4f" % v).rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def path_d(pts, closed):
    d = "M%s %s" % (fmt(pts[0][0][0]), fmt(pts[0][0][1]))
    n = len(pts)
    rng = range(n) if closed else range(n - 1)
    for i in rng:
        a, b = pts[i], pts[(i + 1) % n]
        if a[2] == a[0] and b[1] == b[0]:
            d += "L%s %s" % (fmt(b[0][0]), fmt(b[0][1]))
        else:
            d += "C%s %s %s %s %s %s" % tuple(fmt(v) for v in (a[2][0], a[2][1], b[1][0], b[1][1], b[0][0], b[0][1]))
    return d + ("Z" if closed else "")


def to_svg(icon, size=64):
    defs, body = [], []
    JN = {0: "miter", 1: "miter", 2: "round", 3: "bevel", 4: "miter"}
    CP = {0: "butt", 1: "square", 2: "round"}
    for i, sh in enumerate(icon["shapes"]):
        st = icon["styles"][sh["style"]]
        if st[0] == "solid":
            c = st[1]
            paint = "rgb(%d,%d,%d)" % c[:3]
            pop = c[3] / 255.0
        else:
            _, gt, m, stops = st
            gid = "g%d" % i
            sts = "".join('<stop offset="%s" stop-color="rgb(%d,%d,%d)" stop-opacity="%s"/>'
                          % (fmt(o), c[0], c[1], c[2], fmt(c[3] / 255.0)) for o, c in stops)
            mt = "matrix(%s)" % " ".join(fmt(v) for v in m)
            if gt == GRAD_LINEAR:
                defs.append('<linearGradient id="%s" gradientUnits="userSpaceOnUse" x1="-64" y1="0" x2="64" y2="0" '
                            'gradientTransform="%s">%s</linearGradient>' % (gid, mt, sts))
            else:
                if gt != GRAD_CIRCULAR:
                    pass  # diamond/conic/xy — показываем как круговой
                defs.append('<radialGradient id="%s" gradientUnits="userSpaceOnUse" cx="0" cy="0" r="64" '
                            'gradientTransform="%s">%s</radialGradient>' % (gid, mt, sts))
            paint = "url(#%s)" % gid
            pop = 1.0
        d = "".join(path_d(*icon["paths"][p]) for p in sh["paths"] if p < len(icon["paths"]))
        stroke = next((t for t in sh["transformers"] if t[0] == "stroke"), None)
        if stroke:
            attrs = 'fill="none" stroke="%s" stroke-width="%s" stroke-linejoin="%s" stroke-linecap="%s" ' \
                    'stroke-miterlimit="%d"' % (paint, fmt(stroke[1]), JN.get(stroke[2], "miter"),
                                               CP.get(stroke[3], "butt"), max(1, stroke[4]))
            if pop < 1:
                attrs += ' stroke-opacity="%s"' % fmt(pop)
        else:
            attrs = 'fill="%s"' % paint
            if pop < 1:
                attrs += ' fill-opacity="%s"' % fmt(pop)
        tr = "" if sh["matrix"] == IDENT else ' transform="matrix(%s)"' % " ".join(fmt(v) for v in sh["matrix"])
        body.append('<path%s d="%s" %s/>' % (tr, d, attrs))
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" viewBox="0 0 64 64">'
            '<defs>%s</defs>%s</svg>\n' % (size, size, "".join(defs), "".join(body)))


# ==========================================================================
#  Вывод: rdef / C++
# ==========================================================================
def to_rdef(data, name):
    hexs = data.hex().upper()
    lines = ['\t$"%s"' % hexs[i:i + 64] for i in range(0, len(hexs), 64)]
    return "// %s — сгенерировано svg2hvif %s\nresource vector_icon {\n%s\n};\n" % (name, VERSION, "\n".join(lines))


def to_cpp(data, name):
    ident = "k" + "".join(w.capitalize() for w in re.split(r"[^A-Za-z0-9]+", name) if w) + "Icon"
    rows = []
    for i in range(0, len(data), 12):
        rows.append("\t" + ", ".join("0x%02x" % b for b in data[i:i + 12]) + ",")
    return ("// %s — сгенерировано svg2hvif %s\n// BIconUtils::GetVectorIcon(%s, sizeof(%s), bitmap);\n"
            "static const uint8 %s[] = {\n%s\n};\n" % (name, VERSION, ident, ident, ident, "\n".join(rows)))


# ==========================================================================
#  CLI
# ==========================================================================
def default_font_dirs():
    here = os.path.dirname(os.path.abspath(__file__))
    return [os.path.normpath(os.path.join(here, "..", "..", "fonts")), os.path.join(here, "fonts")]


def main(argv=None):
    ap = argparse.ArgumentParser(description="SVG → HVIF для HeOS")
    ap.add_argument("inputs", nargs="+", help="SVG-файлы или папки (или .hvif с --decode)")
    ap.add_argument("-o", "--out", help="файл (для одного входа) или папка")
    ap.add_argument("--format", default="hvif", help="через запятую: hvif, rdef, cpp, preview (по умолчанию hvif)")
    ap.add_argument("--fonts", action="append", default=None, help="папка со шрифтами для <text> (можно несколько)")
    ap.add_argument("--no-shadows", dest="shadows", action="store_false", help="не имитировать feDropShadow")
    ap.add_argument("--hinting", choices=("auto", "on", "off"), default="auto",
                    help="флаг выравнивания по пикселям: auto = там, где shape-rendering=crispEdges")
    ap.add_argument("--current-color", default="#F2F4F8", help="цвет для currentColor (по умолчанию текст HeOS)")
    ap.add_argument("--decode", action="store_true", help="HVIF → SVG для просмотра")
    ap.add_argument("-q", "--quiet", action="store_true")
    opts = ap.parse_args(argv)
    if opts.fonts is None:
        opts.fonts = default_font_dirs()

    def log(msg):
        if not opts.quiet:
            print(msg)

    if opts.decode:
        for f in opts.inputs:
            data = open(f, "rb").read()
            svg = to_svg(decode(data))
            out = opts.out if (opts.out and len(opts.inputs) == 1) else os.path.splitext(f)[0] + ".preview.svg"
            open(out, "w", encoding="utf-8").write(svg)
            log("%s → %s" % (f, out))
        return 0

    jobs = []
    for inp in opts.inputs:
        if os.path.isdir(inp):
            for root, _, files in os.walk(inp):
                for fn in sorted(files):
                    if fn.lower().endswith(".svg"):
                        src = os.path.join(root, fn)
                        rel = os.path.relpath(src, inp)
                        dst = os.path.join(opts.out or inp, rel) if opts.out else src
                        jobs.append((src, os.path.splitext(dst)[0]))
        else:
            if opts.out and len(opts.inputs) == 1 and not os.path.isdir(opts.out) and opts.out.lower().endswith(".hvif"):
                base = os.path.splitext(opts.out)[0]
            elif opts.out:
                base = os.path.join(opts.out, os.path.splitext(os.path.basename(inp))[0])
            else:
                base = os.path.splitext(inp)[0]
            jobs.append((inp, base))
    formats = [f.strip() for f in opts.format.split(",") if f.strip()]
    errors = 0
    total = 0
    for src, base in jobs:
        warns = []
        try:
            data, b = convert(open(src, encoding="utf-8").read(), opts, warns.append)
            decode(data)  # самопроверка: файл должен разбираться обратно без остатка
        except Exception as e:
            errors += 1
            print("ОШИБКА %s: %s" % (src, e), file=sys.stderr)
            continue
        os.makedirs(os.path.dirname(base) or ".", exist_ok=True)
        name = os.path.basename(base)
        if "hvif" in formats:
            open(base + ".hvif", "wb").write(data)
        if "rdef" in formats:
            open(base + ".rdef", "w", encoding="utf-8").write(to_rdef(data, name))
        if "cpp" in formats:
            open(base + ".icon.h", "w", encoding="utf-8").write(to_cpp(data, name))
        if "preview" in formats:
            open(base + ".preview.svg", "w", encoding="utf-8").write(to_svg(decode(data)))
        total += len(data)
        log("%-40s %5d байт  стилей %d · путей %d · фигур %d" % (
            os.path.relpath(src), len(data), len(b.styles), len(b.paths), len(b.shapes)))
        for w in warns:
            log("    ⚠ " + w)
    log("Готово: %d файлов, %d байт, ошибок %d" % (len(jobs) - errors, total, errors))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
