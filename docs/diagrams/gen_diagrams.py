#!/usr/bin/env python3
"""Generates the architecture diagrams in this folder (SVG).

This script IS the editable source of the drawings: every box, group and arrow
is one call below, with explicit coordinates. Edit, then run

    python3 docs/diagrams/gen_diagrams.py

and commit the regenerated `.svg` next to this file. The `.mermaid` files are the
same diagrams in text form, for renderers that only take Mermaid.

Visual rules (from the reference "Docker vs Kubernetes" style guide):
  * a panel = a solid title bar + a rounded frame with its own colour;
  * a logical boundary = a DASHED box with a pill label centred on its top line;
  * a node = a rounded card, icon on top, name below, optional dark code block;
  * control flow = dashed arrow in the panel colour; data / client flow = dashed
    green; labels sit in free space, never on a line; no arrow crosses a card;
  * at most two colours per panel plus neutral grey; the lowest layer is a wide
    grey band at the bottom.

Every element drawn here is listed, with the file where it was confirmed, in
the "Diagrams" section of ARCHITECTURE.md. Labels are pt-AO (pre-AO90), like
the operator-facing docs; identifiers and comments stay in English (LANG-01).

stdlib only.
"""
from __future__ import annotations

from pathlib import Path
from xml.sax.saxutils import escape

HERE = Path(__file__).resolve().parent

# Palette: one colour per panel + green for data/client flow + neutral grey.
TEAL = "#0f766e"
TEAL_BG = "#f0fdfa"
PURPLE = "#6d28d9"
PURPLE_BG = "#faf7ff"
GREEN = "#15803d"
GREY = "#64748b"
GREY_LINE = "#94a3b8"
GREY_BG = "#f1f5f9"
INK = "#0f172a"
CODE_BG = "#1e293b"
CODE_FG = "#e2e8f0"

FONT = "Inter, 'Segoe UI', 'Helvetica Neue', Arial, sans-serif"
MONO = "'JetBrains Mono', 'DejaVu Sans Mono', ui-monospace, monospace"

# --- icons: one line set, 24x24 grid, stroke 1.75, round caps --------------------
ICONS = {
    "terminal": '<rect x="2.5" y="4" width="19" height="16" rx="2.5"/><path d="M6.5 9.5l3 2.5-3 2.5M12 15h5"/>',
    "server": '<rect x="3" y="3.5" width="18" height="7" rx="2"/><rect x="3" y="13.5" width="18" height="7" rx="2"/><path d="M7 7h.01M7 17h.01M11 7h6M11 17h6"/>',
    "layers": '<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 12.5l9 5 9-5"/><path d="M3 16.5l9 5 9-5"/>',
    "cycle": '<circle cx="12" cy="12" r="9"/><path d="M10 8.5l5 3.5-5 3.5z"/>',
    "cube": '<path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z"/><path d="M4 7.5l8 4.5 8-4.5M12 12v9"/>',
    "file": '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h4"/>',
    "json": '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/><path d="M10 11.5c-1 0-1.3.5-1.3 1.3v.6c0 .6-.3.9-.9.9.6 0 .9.3.9.9v.6c0 .8.3 1.3 1.3 1.3M14 11.5c1 0 1.3.5 1.3 1.3v.6c0 .6.3.9.9.9-.6 0-.9.3-.9.9v.6c0 .8-.3 1.3-1.3 1.3"/>',
    "eye": '<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    "log": '<rect x="4" y="3" width="16" height="18" rx="2"/><path d="M8 8h8M8 12h8M8 16h5"/>',
    "swap": '<path d="M4 9h15l-3.5-3.5M20 15H5l3.5 3.5"/>',
    "trash": '<path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13M10 11v5M14 11v5"/>',
    "pin": '<path d="M12 21s-7-6.2-7-11a7 7 0 0 1 14 0c0 4.8-7 11-7 11z"/><circle cx="12" cy="10" r="2.5"/>',
    "sliders": '<path d="M4 6h16M4 12h16M4 18h16"/><circle cx="9" cy="6" r="2" fill="#fff"/><circle cx="15" cy="12" r="2" fill="#fff"/><circle cx="7" cy="18" r="2" fill="#fff"/>',
    "cpu": '<rect x="6" y="6" width="12" height="12" rx="1.5"/><rect x="9.5" y="9.5" width="5" height="5"/><path d="M9.5 2.5V6M14.5 2.5V6M9.5 18v3.5M14.5 18v3.5M2.5 9.5H6M2.5 14.5H6M18 9.5h3.5M18 14.5h3.5"/>',
    "monitor": '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M6.5 11h2.5l2-3 2 6 2-3h2.5M9 21h6M12 17v4"/>',
    "database": '<ellipse cx="12" cy="5.5" rx="8" ry="3"/><path d="M4 5.5v13c0 1.7 3.6 3 8 3s8-1.3 8-3v-13M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"/>',
    "calendar": '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/>',
    "plug": '<path d="M9 2.5V7M15 2.5V7M6 7h12v4a6 6 0 0 1-12 0zM12 17v4.5"/>',
    "laptop": '<rect x="4" y="4.5" width="16" height="11" rx="1.5"/><path d="M2 19.5h20"/>',
    "door": '<path d="M14 3.5h4.5a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H14M9.5 16.5L14 12l-4.5-4.5M14 12H3.5"/>',
    "share": '<circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="M8.2 10.8l7.6-3.6M8.2 13.2l7.6 3.6"/>',
    "chip": '<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M8 8h8v8H8z"/>',
}


def text_w(s: str, size: float, mono: bool = False) -> float:
    """Rough rendered width, enough to size pills and code blocks."""
    return len(s) * size * (0.61 if mono else 0.56)


class Svg:
    def __init__(self, w: int, h: int, title: str):
        self.w, self.h, self.title = w, h, title
        self.parts: list[str] = []

    def add(self, s: str) -> None:
        self.parts.append(s)

    # -- primitives ---------------------------------------------------------------
    def text(self, x, y, s, size=12, color=INK, weight=400, anchor="middle",
             mono=False, italic=False, halo=False):
        style = ' font-style="italic"' if italic else ""
        h = ' stroke="#fff" stroke-width="4" stroke-linejoin="round" paint-order="stroke"' if halo else ""
        self.add(
            f'<text x="{x}" y="{y}" font-family="{MONO if mono else FONT}" font-size="{size}" '
            f'font-weight="{weight}" fill="{color}" text-anchor="{anchor}"{style}{h}>{escape(s)}</text>'
        )

    def icon(self, name, cx, cy, size, color):
        k = size / 24
        self.add(
            f'<g transform="translate({cx - size / 2},{cy - size / 2}) scale({k})" fill="none" '
            f'stroke="{color}" stroke-width="{1.75 / k * k:.2f}" stroke-linecap="round" '
            f'stroke-linejoin="round">{ICONS[name]}</g>'
        )

    def titlebar(self, x, y, w, h, label, color):
        self.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{color}"/>')
        self.text(x + w / 2, y + h / 2 + 9, label, size=26, color="#fff", weight=700)

    def frame(self, x, y, w, h, color, bg):
        self.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14" fill="{bg}" '
                 f'stroke="{color}" stroke-width="1.6"/>')

    def group(self, x, y, w, h, label, color):
        """Dashed logical boundary with a pill label centred on the top line."""
        self.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10" fill="none" '
                 f'stroke="{color}" stroke-width="1.4" stroke-dasharray="5 4"/>')
        pw = text_w(label, 11.5) + 22
        self.add(f'<rect x="{x + w / 2 - pw / 2}" y="{y - 11}" width="{pw}" height="22" rx="11" '
                 f'fill="#fff" stroke="{color}" stroke-width="1.4"/>')
        self.text(x + w / 2, y + 4.5, label, size=11.5, color=INK, weight=700)

    def pill(self, cx, cy, label, color, bg, size=12):
        pw = text_w(label, size) + 28
        self.add(f'<rect x="{cx - pw / 2}" y="{cy - 13}" width="{pw}" height="26" rx="13" '
                 f'fill="{bg}" stroke="{color}" stroke-width="1.2"/>')
        self.text(cx, cy + size * 0.36, label, size=size, color=color, weight=700)

    def card(self, x, y, w, h, icon, name, color, sub=(), code=(), bg="#fff",
             name_color=INK, layout="v", muted=False, sub_size=9.8):
        """A node. layout "v": icon on top, name below; "h": icon left (bars).

        `muted` draws an OPTIONAL piece (born on demand, dies with its resource):
        lighter fill, faded border and name, so the always-there path stands out.
        """
        op = ' stroke-opacity="0.45"' if muted else ""
        if muted:
            bg, name_color = "#fbfaff", "#475569"
        self.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10" fill="{bg}" '
                 f'stroke="{color}" stroke-width="1.4"{op}/>')
        if layout == "h":
            self.icon(icon, x + 22, y + h / 2, 20, color)
            self.text(x + 42, y + h / 2 + 4.5, name, size=12.5, weight=600, anchor="start",
                      color=name_color)
            return
        cy = y + 12 + 13
        self.icon(icon, x + w / 2, cy, 26, color)
        ty = cy + 13 + 17
        self.text(x + w / 2, ty, name, size=12.5, weight=700, color=name_color)
        ty += 4
        if code:
            cw = max(text_w(c, 10, mono=True) for c in code) + 16
            cw = min(cw, w - 12)
            ch = 8 + 13 * len(code)
            self.add(f'<rect x="{x + w / 2 - cw / 2}" y="{ty + 3}" width="{cw}" height="{ch}" '
                     f'rx="4" fill="{CODE_BG}"/>')
            for i, c in enumerate(code):
                self.text(x + w / 2, ty + 3 + 15 + 13 * i, c, size=10, color=CODE_FG, mono=True)
            ty += ch + 4
        for i, s in enumerate(sub):
            self.text(x + w / 2, ty + 13 + 12 * i, s, size=sub_size, color=GREY)

    def band(self, x, y, w, h, title, detail=""):
        self.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" fill="#e2e8f0" '
                 f'stroke="{GREY_LINE}" stroke-width="1"/>')
        if detail:
            self.text(x + w / 2, y + h / 2 - 3, title, size=13, weight=700, color="#334155")
            self.text(x + w / 2, y + h / 2 + 14, detail, size=11, color="#475569")
        else:
            self.text(x + w / 2, y + h / 2 + 5, title, size=13, weight=700, color="#334155")

    def arrow(self, pts, color, both=False, width=1.6, d=None, opacity=1.0):
        """Dashed arrow along an orthogonal polyline (or a raw path `d`)."""
        mid = color.lstrip("#")
        path = d or "M " + " L ".join(f"{x} {y}" for x, y in pts)
        start = f' marker-start="url(#a{mid})"' if both else ""
        op = f' opacity="{opacity}"' if opacity < 1 else ""
        self.add(f'<path d="{path}" fill="none" stroke="{color}" stroke-width="{width}" '
                 f'stroke-dasharray="6 4" stroke-linecap="round" stroke-linejoin="round"'
                 f'{start} marker-end="url(#a{mid})"{op}/>')

    def render(self) -> str:
        colors = [TEAL, PURPLE, GREEN, GREY]
        markers = "".join(
            f'<marker id="a{c.lstrip("#")}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" '
            f'markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="{c}"/></marker>'
            for c in colors
        )
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}" height="{self.h}" '
            f'viewBox="0 0 {self.w} {self.h}" role="img" aria-label="{escape(self.title)}">\n'
            f'<title>{escape(self.title)}</title>\n<defs>{markers}</defs>\n'
            f'<rect width="{self.w}" height="{self.h}" fill="#fff"/>\n'
            + "\n".join(self.parts) + "\n</svg>\n"
        )


# --- Diagram A: Docker vs Delonix ----------------------------------------------------
def docker_vs_delonix() -> Svg:
    s = Svg(1240, 950, "Docker vs Delonix — arquitectura interna do Delonix Engine")
    s.titlebar(20, 14, 380, 46, "Docker", TEAL)
    s.text(420, 47, "vs", size=24, weight=700, color=INK)
    s.titlebar(440, 14, 780, 46, "Delonix", PURPLE)

    # Left panel: the well-known chain (reference image), with the per-container
    # containerd-shim drawn too — it is Docker's counterpart of our supervisor, and
    # leaving it out would flatter the comparison. The Docker CLI sits OUTSIDE the
    # host on purpose: it is a client of dockerd's socket.
    s.frame(20, 72, 380, 858, TEAL, TEAL_BG)
    s.card(115, 96, 190, 128, "terminal", "Docker CLI", TEAL, code=("docker run nginx",),
           sub=("cliente do socket do dockerd",))
    s.group(40, 258, 340, 548, "Host", TEAL)
    s.card(135, 290, 150, 80, "server", "dockerd", TEAL, sub=("daemon residente",))
    s.card(135, 392, 150, 80, "layers", "containerd", TEAL, sub=("daemon residente",))
    s.card(135, 494, 150, 80, "eye", "containerd-shim", TEAL, sub=("um por container",))
    s.card(135, 596, 150, 80, "cycle", "runc", TEAL, sub=("OCI runtime",))
    s.card(135, 698, 150, 86, "cube", "Container", TEAL)
    # Elbow on the right, as in the reference: a straight drop would cross the pill.
    s.arrow([(305, 160), (340, 160), (340, 330), (285, 330)], TEAL)
    for y in (370, 472, 574, 676):
        s.arrow([(210, y), (210, y + 22)], TEAL)
    s.text(210, 826, "5 saltos · 2 daemons residentes", size=11.5, color=GREY, weight=600)
    s.band(40, 842, 340, 56, "Linux kernel")

    # Right panel: Delonix. The CLI runs ON the host: it clones the container itself.
    s.frame(440, 72, 780, 858, PURPLE, PURPLE_BG)
    s.pill(830, 96, "sem daemon: nenhum processo residente obrigatório", PURPLE, "#ede9fe")
    s.group(452, 126, 756, 680, "Host", PURPLE)

    s.group(466, 158, 200, 146, "Estado: $DELONIX_ROOT (JSON)", GREEN)
    s.card(480, 180, 172, 112, "json", "um JSON por registo", GREEN,
           code=("containers/ images/ cri/",), sub=("escrita atómica + flock",))

    s.card(742, 158, 190, 146, "terminal", "delonix CLI", PURPLE,
           code=("delonix container run …",), sub=("efémera: termina após", "cada comando"))
    # State: read and written by the CLI (data flow, green). Label in two short
    # lines so it clears both ends of the gap.
    s.arrow([(666, 232), (742, 232)], GREEN, both=True)
    s.text(704, 211, "lê e", size=10, color=GREEN, weight=600)
    s.text(704, 223, "escreve", size=10, color=GREEN, weight=600)

    # The message: in the foreground nothing sits between the CLI and the container.
    s.card(747, 698, 180, 86, "cube", "Container", PURPLE)
    s.arrow([(837, 304), (837, 698)], PURPLE, width=2.4)
    s.text(822, 380, "primeiro plano:", size=10.5, color=GREY, weight=600, anchor="end")
    s.text(822, 397, "clone + namespaces", size=12, weight=700, color=PURPLE, anchor="end")
    s.text(822, 414, "+ pivot_root + cgroups v2", size=12, weight=700, color=PURPLE, anchor="end")
    s.text(822, 430, "(delonix-linux)", size=10.5, color=GREY, anchor="end")

    # Detached (-d): the CLI forks a supervisor (a copy of itself) and the supervisor
    # does the same clone, staying as the container's parent. Drawn as a path of its
    # own so the "1 hop" above is not contradicted by the drawing.
    s.card(552, 470, 120, 104, "eye", "supervisor", PURPLE,
           sub=("fork da CLI", "um por container -d"))
    s.arrow([(760, 304), (760, 326), (612, 326), (612, 470)], PURPLE)
    s.text(690, 346, "em -d: fork", size=10, color=PURPLE, weight=600)
    s.arrow([(612, 574), (612, 741), (747, 741)], PURPLE)
    s.text(680, 733, "clone", size=10, color=PURPLE, weight=600)

    # Optional per-container helpers (muted: born on demand, die with the container).
    s.group(998, 158, 204, 234, "Por container (opcional)", PURPLE)
    s.card(1008, 180, 90, 100, "log", "log shim", PURPLE, sub=("fork; escreve", "o log"),
           muted=True, sub_size=9)
    s.card(1106, 180, 90, 100, "swap", "slirp4netns", PURPLE,
           sub=("um por container", "--net private"), muted=True, sub_size=9)
    s.card(1008, 290, 188, 90, "trash", "watcher --rm", PURPLE,
           sub=("só com --no-supervisor",), muted=True)
    s.arrow([(932, 205), (998, 205)], PURPLE)

    # Rootless network plane, brought up on demand (ref-counted, not a daemon).
    s.group(998, 422, 204, 222, "Rede rootless (sob procura)", PURPLE)
    s.card(1008, 444, 90, 92, "pin", "netns pin", PURPLE, sub=("userns + netns",), muted=True,
           sub_size=9)
    s.card(1106, 444, 90, 92, "share", "slirp4netns", PURPLE, sub=("único,", "partilhado"),
           muted=True, sub_size=9)
    s.card(1008, 546, 188, 88, "sliders", "netns control", PURPLE,
           sub=("bridge delonix0 · nft · DHCP · DNS",), muted=True)
    s.arrow([(932, 262), (966, 262), (966, 533), (998, 533)], PURPLE)

    # Another kind of workload, also on demand: one process (or domain) per VM.
    s.card(1008, 676, 188, 108, "cpu", "microVM", PURPLE,
           sub=("VmBackend: Cloud Hypervisor", "ou libvirt · vive com a VM"), muted=True)
    s.arrow([(932, 290), (950, 290), (950, 730), (1008, 730)], PURPLE)

    s.text(830, 826, "primeiro plano: 1 salto · em -d: 2 (fork + clone) · nenhum daemon",
           size=11.5, color=GREY, weight=600)
    s.band(452, 842, 756, 56, "Linux kernel", "namespaces · cgroups v2 · nftables · seccomp")
    s.text(830, 918, "cartão esbatido = opcional: nasce quando é preciso e morre com o recurso",
           size=10, color=GREY, italic=True)
    return s


# --- Diagram B: Kubernetes with Delonix ----------------------------------------------
def kubernetes_with_delonix() -> Svg:
    s = Svg(980, 1225, "Kubernetes com Delonix — delonix-cri no lugar do containerd")
    s.titlebar(20, 14, 940, 46, "Kubernetes com Delonix", PURPLE)
    s.frame(20, 72, 940, 1135, PURPLE, PURPLE_BG)
    s.pill(490, 96, "sem containerd: o kubelet fala directamente com o delonix-cri",
           PURPLE, "#ede9fe")

    # Kubernetes itself, grey: context, not Delonix code.
    s.card(55, 96, 180, 124, "file", "Deployment", GREY, code=("replicas: 3", "image: nginx"))
    s.arrow([(145, 220), (145, 292)], GREY)
    s.text(155, 244, "kubectl apply", size=10.5, color=GREY, weight=600, anchor="start")

    s.group(45, 262, 600, 180, "Control Plane (contexto)", GREY)
    s.card(70, 292, 150, 128, "server", "API Server", GREY)
    s.card(290, 290, 330, 38, "database", "etcd", GREY, layout="h")
    s.card(290, 338, 330, 38, "sliders", "Controller Manager", GREY, layout="h")
    s.card(290, 386, 330, 38, "calendar", "Scheduler", GREY, layout="h")
    for y in (309, 357, 405):
        s.arrow([(220, y), (290, y)], GREY, both=True, width=1.3)

    # delonix cluster: bootstraps every node over ssh + kubeadm.
    s.card(690, 262, 240, 168, "terminal", "delonix cluster", PURPLE,
           code=("delonix cluster apply -f …",), sub=("ssh + kubeadm init / join", "em cada nó"))

    def node(y0: int, label: str, pods: list[str]) -> None:
        s.group(45, y0, 775, 275, label, PURPLE)
        s.card(80, y0 + 35, 130, 92, "monitor", "kubelet", GREY)
        s.card(270, y0 + 25, 260, 112, "plug", "delonix-cri", PURPLE,
               code=("unix:///run/delonix-cri.sock",), sub=("no lugar do containerd",))
        s.arrow([(210, y0 + 81), (270, y0 + 81)], PURPLE)
        s.text(240, y0 + 71, "gRPC CRI", size=10, color=PURPLE, weight=600)
        s.card(640, y0 + 35, 160, 92, "json", "$DELONIX_ROOT/cri", GREEN,
               sub=("JSON · reconcile_status",))
        s.arrow([(530, y0 + 81), (640, y0 + 81)], GREEN, both=True)
        s.text(585, y0 + 71, "load_reconciled", size=10, color=GREEN, weight=600)
        # cri -> pods: the cri hands each start to a fresh `delonix __apirun`.
        xs = [285 + 135 * i for i in range(len(pods))]
        centers = [x + 55 for x in xs]
        for x, name in zip(xs, pods):
            s.card(x, y0 + 190, 110, 62, "cube", name, PURPLE, layout="h")
        top = y0 + 137
        if len(pods) == 1:
            stem = centers[0]
            s.arrow([(stem, top), (stem, y0 + 190)], PURPLE)
            label_y = y0 + 166
        else:
            stem = (centers[0] + centers[-1]) // 2
            branch = y0 + 168
            s.add(f'<path d="M {stem} {top} L {stem} {branch} M {centers[0]} {branch} '
                  f'L {centers[-1]} {branch}" fill="none" stroke="{PURPLE}" stroke-width="1.6" '
                  f'stroke-dasharray="6 4" stroke-linecap="round"/>')
            for c in centers:
                s.arrow([(c, branch), (c, y0 + 190)], PURPLE)
            label_y = y0 + 156
        # Beside the stem, between the cri card and the pods — not at pod level,
        # where it read as a pod name.
        s.text(stem + 8, label_y, "delonix __apirun", size=10, color=PURPLE, weight=600,
               anchor="start", mono=True)

    node(482, "Node 1", ["Pod A", "Pod B"])
    node(789, "Node 2", ["Pod C"])

    # API Server -> kubelets (Kubernetes, grey): straight down, and the left trunk.
    s.arrow([(145, 420), (145, 517)], GREY)
    s.arrow([(70, 400), (32, 400), (32, 870), (80, 870)], GREY)

    # delonix cluster -> nodes ("provisiona"), right trunk. The heads land on the
    # node border below the state card, so they never read as pointing at it.
    s.arrow([(880, 430), (880, 632), (820, 632)], PURPLE)
    s.arrow([(880, 632), (880, 939), (820, 939)], PURPLE)
    s.text(887, 470, "provisiona", size=10.5, color=PURPLE, weight=600, anchor="start")

    # Client traffic (green). Ingress and Service are Kubernetes objects: grey cards.
    s.card(55, 1115, 120, 64, "laptop", "Client", GREY, layout="h")
    s.card(235, 1115, 140, 64, "door", "Ingress", GREY, layout="h")
    s.card(435, 1115, 140, 64, "share", "Service", GREY, layout="h")
    s.arrow([(175, 1147), (235, 1147)], GREEN)
    s.arrow([(375, 1147), (435, 1147)], GREEN)
    # Service -> Pod C (up through Node 2's bottom).
    s.arrow([(505, 1115), (505, 1090), (340, 1090), (340, 1041)], GREEN)
    # Service -> Pod B (Node 1): right side, hopping over the provisioning trunk.
    # The one arrow-on-arrow crossing of this drawing; it cannot be removed while
    # three sources (API Server, delonix cluster, Client) each reach both nodes.
    # Thinner and lighter: it is the longest line and must not pull the eye.
    y = 703
    s.arrow(None, GREEN, width=1.3, opacity=0.7,
            d=(f"M 575 1147 L 945 1147 L 945 {y} L 886 {y} "
               f"A 6 6 0 0 0 874 {y} L 530 {y}"))
    return s


def main() -> None:
    for name, build in (("delonix-vs-docker", docker_vs_delonix),
                        ("delonix-kubernetes", kubernetes_with_delonix)):
        out = HERE / f"{name}.svg"
        out.write_text(build().render(), encoding="utf-8")
        print(f"wrote {out.relative_to(HERE.parent.parent)}")


if __name__ == "__main__":
    main()
