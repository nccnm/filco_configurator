r"""Render docs/PLAN.md to docs/plan.html: one self-contained page in the style of the "M10SP layer setup" page.

Usage (from the repo root, Python 3.11+, stdlib only):
    python -I tools\gen_plan_html.py [input.md] [output.html]     default: docs/PLAN.md -> docs/plan.html

Supports the Markdown subset used by PLAN.md: ATX headings, paragraphs, bullet and numbered lists (with indented
continuation blocks such as tables), pipe tables, fenced code blocks, inline code, bold and links.
"""
import html
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def esc(s):
    return html.escape(s, quote=True)


def inline(text):
    parts = re.split(r"(`[^`]*`)", text)
    out = []
    for p in parts:
        if len(p) >= 2 and p.startswith("`") and p.endswith("`"):
            out.append(f"<code>{esc(p[1:-1])}</code>")
            continue
        s = esc(p)
        s = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
        s = re.sub(r"\[([^\]]+)\]\((https?://[^)\s]+)\)", r'<a href="\2">\1</a>', s)
        out.append(s)
    return "".join(out)


def slug(text, used):
    base = re.sub(r"[^\w]+", "-", text.lower(), flags=re.UNICODE).strip("-") or "muc"
    s, n = base, 2
    while s in used:
        s, n = f"{base}-{n}", n + 1
    used.add(s)
    return s


BULLET = re.compile(r"^(\s*)([-*]|\d+\.)\s+(.*)$")
TABLE_SEP = re.compile(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$")


def split_row(line):
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|") and not s.endswith("\\|"):
        s = s[:-1]
    cells, cur, i = [], "", 0
    in_code = False
    while i < len(s):
        c = s[i]
        if c == "`":
            in_code = not in_code
        if c == "\\" and i + 1 < len(s) and s[i + 1] == "|":
            cur += "|"
            i += 2
            continue
        if c == "|" and not in_code:
            cells.append(cur.strip())
            cur = ""
        else:
            cur += c
        i += 1
    cells.append(cur.strip())
    return cells


def parse_blocks(lines, toc, used, top=True):
    out = []
    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        if not line.strip():
            i += 1
            continue
        if line.startswith("```"):
            lang = line[3:].strip()
            j = i + 1
            body = []
            while j < n and not lines[j].startswith("```"):
                body.append(lines[j])
                j += 1
            cls = f' class="lang-{esc(lang)}"' if lang else ""
            out.append(f"<pre{cls}><code>{esc(chr(10).join(body))}</code></pre>")
            i = j + 1
            continue
        if line.startswith("<!--"):
            i += 1
            continue
        m = re.match(r"^(#{1,4})\s+(.*)$", line)
        if m and top:
            level = len(m.group(1))
            text = m.group(2).strip()
            sid = slug(text, used)
            if level == 2:
                toc.append((sid, text))
            out.append(f'<h{level} id="{sid}">{inline(text)}</h{level}>')
            i += 1
            continue
        if line.lstrip().startswith("|") and i + 1 < n and TABLE_SEP.match(lines[i + 1]):
            head = split_row(line)
            j = i + 2
            rows = []
            while j < n and lines[j].lstrip().startswith("|"):
                rows.append(split_row(lines[j]))
                j += 1
            th = "".join(f"<th>{inline(c)}</th>" for c in head)
            body = "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in rows)
            out.append(f'<div class="tw"><table><thead><tr>{th}</tr></thead><tbody>{body}</tbody></table></div>')
            i = j
            continue
        m = BULLET.match(line)
        if m and not m.group(1):
            ordered = m.group(2)[0].isdigit()
            items = []
            while i < n:
                m = BULLET.match(lines[i])
                if not m or m.group(1) or (m.group(2)[0].isdigit()) != ordered:
                    break
                width = len(m.group(2)) + 1
                body = [m.group(3)]
                j = i + 1
                while j < n:
                    nxt = lines[j]
                    if not nxt.strip():
                        # blank line: continue only if the next non-blank line is indented continuation
                        k = j
                        while k < n and not lines[k].strip():
                            k += 1
                        if k < n and lines[k].startswith(" " * width):
                            body.extend([""] * (k - j))
                            j = k
                            continue
                        break
                    if nxt.startswith(" " * width):
                        body.append(nxt[width:])
                        j += 1
                        continue
                    if BULLET.match(nxt) is None and not nxt.startswith(("#", "|", "```")) and nxt.startswith(" "):
                        body.append(nxt.strip())
                        j += 1
                        continue
                    break
                items.append(body)
                i = j
            tag = "ol" if ordered else "ul"
            lis = []
            for body in items:
                first_para = []
                k = 0
                while k < len(body) and body[k].strip() and not body[k].lstrip().startswith(("|", "```")):
                    first_para.append(body[k].strip())
                    k += 1
                rest = parse_blocks(body[k:], toc, used, top=False) if k < len(body) else ""
                lis.append(f"<li>{inline(' '.join(first_para))}{rest}</li>")
            out.append(f"<{tag}>{''.join(lis)}</{tag}>")
            continue
        para = []
        while i < n and lines[i].strip() and not lines[i].startswith(("```", "#")) \
                and not (lines[i].lstrip().startswith("|") and i + 1 < n and TABLE_SEP.match(lines[i + 1])) \
                and not (BULLET.match(lines[i]) and not BULLET.match(lines[i]).group(1)):
            para.append(lines[i].strip())
            i += 1
        if para:
            out.append(f"<p>{inline(' '.join(para))}</p>")
        else:
            i += 1
    return "".join(out)


CSS = """
:root {
  --paper: #fbfbfa; --ink: #1d2126; --line: #9aa1a8; --faint: #b9bec3; --soft: #eef0f1;
  --c-l1: #b4532a; --c-l2: #2f63b0; --c-l3: #2e7d4f; --c-l4: #6a4aa5; --c-fn: #8a6a12;
  --font-body: "IBM Plex Sans", "Segoe UI", system-ui, sans-serif;
  --font-key: "IBM Plex Sans Condensed", "Arial Narrow", "Segoe UI", sans-serif;
  --font-mono: "IBM Plex Mono", Consolas, monospace;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --paper: #16191c; --ink: #e6e8ea; --line: #69717a; --faint: #4a5058; --soft: #1f2327;
    --c-l1: #e08a5c; --c-l2: #7fa8e8; --c-l3: #6cc28f; --c-l4: #b49ae6; --c-fn: #d4b45a;
    color-scheme: dark;
  }
}
:root[data-theme="dark"] {
  --paper: #16191c; --ink: #e6e8ea; --line: #69717a; --faint: #4a5058; --soft: #1f2327;
  --c-l1: #e08a5c; --c-l2: #7fa8e8; --c-l3: #6cc28f; --c-l4: #b49ae6; --c-fn: #d4b45a;
  color-scheme: dark;
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--paper); color: var(--ink); font: 15px/1.55 var(--font-body); }
main { max-width: 1120px; margin: 0 auto; padding: 28px 20px 56px; }
h1 { font: 600 26px/1.2 var(--font-key); margin: 0 0 10px; text-wrap: balance; letter-spacing: .01em; }
h2 { font: 600 20px/1.3 var(--font-key); margin: 40px 0 10px; padding-bottom: 6px; border-bottom: 1px solid var(--line);
  display: flex; align-items: center; gap: 9px; }
h2::before { content: ""; width: 10px; height: 10px; border-radius: 50%; flex: none; background: var(--acc, var(--c-l1)); }
h3 { font: 600 16.5px/1.3 var(--font-key); margin: 26px 0 8px; color: var(--acc, var(--c-l1)); }
h4 { font: 600 15px var(--font-key); margin: 20px 0 6px; }
section:nth-of-type(4n+1) { --acc: var(--c-l1); } section:nth-of-type(4n+2) { --acc: var(--c-l2); }
section:nth-of-type(4n+3) { --acc: var(--c-l3); } section:nth-of-type(4n+4) { --acc: var(--c-l4); }
p, li { max-width: 95ch; }
p { margin: 8px 0; }
ul, ol { margin: 8px 0; padding-left: 22px; display: grid; gap: 4px; }
li > ul, li > ol, li > .tw, li > pre { margin-top: 6px; }
a { color: var(--ink); text-underline-offset: 2px; }
code { font: 13px var(--font-mono); background: var(--soft); padding: 1px 4px; border-radius: 4px; }
pre { background: transparent; border: 1px solid var(--line); border-radius: 7px; padding: 12px 14px; overflow-x: auto;
  margin: 10px 0; }
pre code { background: none; padding: 0; font: 12.5px/1.45 var(--font-mono); white-space: pre; }
.tw { overflow-x: auto; margin: 10px 0; }
table { border-collapse: collapse; width: 100%; font-size: 14px; }
th { text-align: left; font: 500 12.5px var(--font-key); opacity: .7; padding: 0 14px 6px 0;
  border-bottom: 1px solid var(--line); vertical-align: bottom; }
td { padding: 7px 14px 7px 0; vertical-align: top; border-bottom: 1px solid var(--soft); }
td code, th code { font-size: 12.5px; }
nav.toc { display: flex; flex-wrap: wrap; gap: 8px; margin: 16px 0 8px; }
nav.toc a { text-decoration: none; border: 1px solid var(--line); border-radius: 6px; padding: 3px 10px;
  font: 500 13.5px var(--font-key); }
nav.toc a:hover, nav.toc a:focus-visible { border-color: var(--ink); outline: none; }
.meta { font: 500 13.5px var(--font-key); opacity: .72; margin: 0; }
strong { font-weight: 600; }
@media (max-width: 600px) { main { padding: 20px 16px 40px; } h1 { font-size: 22px; } }
"""


def render(md_text):
    lines = md_text.replace("\r\n", "\n").split("\n")
    title_line = next(l for l in lines if l.startswith("# "))
    title = title_line[2:].strip()
    body_lines = lines[lines.index(title_line) + 1:]
    # Split into sections at each H2 so each gets its own accent colour.
    chunks, cur = [], []
    for l in body_lines:
        if l.startswith("## ") and cur:
            chunks.append(cur)
            cur = []
        cur.append(l)
    chunks.append(cur)
    toc, used = [], set()
    intro = parse_blocks(chunks[0], toc, used) if chunks and not chunks[0][0].startswith("## ") else ""
    secs = [f"<section>{parse_blocks(c, toc, used)}</section>"
            for c in (chunks[1:] if intro else chunks)]
    nav = "".join(f'<a href="#{sid}">{inline(t)}</a>' for sid, t in toc)
    plain_title = re.sub(r"`([^`]*)`", r"\1", title)
    return f"""<!DOCTYPE html>
<html lang="vi">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{esc(plain_title)}</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans+Condensed:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600&family=IBM+Plex+Mono:wght@400&display=swap" rel="stylesheet">
<style>{CSS}</style>
</head>
<body>
<main>
<h1>{inline(title)}</h1>
<nav class="toc" aria-label="Mục lục">{nav}</nav>
{intro}
{''.join(secs)}
</main>
</body>
</html>
"""


def main():
    src = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "docs" / "PLAN.md"
    dst = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "docs" / "plan.html"
    dst.write_text(render(src.read_text(encoding="utf-8")), encoding="utf-8", newline="\n")
    print(f"wrote {dst}")


if __name__ == "__main__":
    main()
