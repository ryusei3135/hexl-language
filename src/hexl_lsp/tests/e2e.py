#!/usr/bin/env python3
"""hexl_lsp の E2E テスト: `cargo build --release` のあとに `python3 tests/e2e.py`"""
import json, subprocess, sys, pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "release" / "hexl_lsp"
PROJ = ROOT / "examples" / "project"

p = subprocess.Popen([str(BIN)], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
_id = 0
notes = []

def send(o):
    b = json.dumps(o).encode()
    p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b)
    p.stdin.flush()

def read():
    n = 0
    while True:
        l = p.stdout.readline().strip()
        if not l:
            break
        if l.startswith(b"Content-Length:"):
            n = int(l.split(b":")[1])
    return json.loads(p.stdout.read(n))

def request(method, params):
    global _id
    _id += 1
    send({"jsonrpc": "2.0", "id": _id, "method": method, "params": params})
    while True:
        m = read()
        if m.get("id") == _id:
            assert "error" not in m, m
            return m["result"]
        notes.append(m)

def notify(method, params):
    send({"jsonrpc": "2.0", "method": method, "params": params})

def uri(path):
    return pathlib.Path(path).as_uri()

fails = 0
def check(name, cond, info=""):
    global fails
    print(("ok   " if cond else "FAIL ") + name + ("" if cond else f"  {info}"))
    if not cond:
        fails += 1

def open_doc(path, text):
    notify("textDocument/didOpen", {"textDocument": {"uri": uri(path), "text": text}})

def td(path, line, ch, **kw):
    return {"textDocument": {"uri": uri(path)}, "position": {"line": line, "character": ch}, **kw}

# ---- initialize
r = request("initialize", {"workspaceFolders": [{"uri": uri(PROJ), "name": "p"}], "capabilities": {}})
caps = r["capabilities"]
for k in ["completionProvider", "hoverProvider", "definitionProvider", "referencesProvider",
          "renameProvider", "foldingRangeProvider", "semanticTokensProvider", "signatureHelpProvider"]:
    check("capability " + k, k in caps)
notify("initialized", {})

MAIN = PROJ / "main.hexl"
open_doc(MAIN, MAIN.read_text())
request("workspace/symbol", {"query": "zzz"})  # 通知を吐き出させる

# ---- 診断
diag = [m for m in notes if m.get("method") == "textDocument/publishDiagnostics"
        and m["params"]["uri"] == uri(MAIN)][-1]["params"]["diagnostics"]
msgs = [d["message"] for d in diag]
check("missing include warned", any("missing.hexl" in m for m in msgs), msgs)
check("existing/std include not warned", not any("geom.hexl" in m or "vec.hexl" in m for m in msgs), msgs)
check("continue outside loop", any("continue" in m for m in msgs), msgs)
check("no break warning in loop", not any("`break`" in m for m in msgs), msgs)
check("no errors", all(d["severity"] != 1 for d in diag), diag)

# ---- documentSymbol(階層)
syms = request("textDocument/documentSymbol", td(MAIN, 0, 0))
names = [s["name"] for s in syms]
check("top-level symbols", names == ["N", "Vec2", "main"], names)
impl = syms[1]
check("impl has method child", [c["name"] for c in impl["children"]] == ["twice"], impl)

# ---- 定義
d = request("textDocument/definition", td(MAIN, 5, 6))
check("impl Vec2 -> struct in geom.hexl", d and d[0]["uri"].endswith("lib/geom.hexl") and d[0]["range"]["start"]["line"] == 1, d)
d = request("textDocument/definition", td(MAIN, 7, 18))
check("self->len -> method in geom.hexl", d and d[0]["uri"].endswith("lib/geom.hexl") and d[0]["range"]["start"]["line"] == 5, d)
d = request("textDocument/definition", td(MAIN, 13, 14))
check("geom::add -> geom.hexl", d and d[0]["uri"].endswith("lib/geom.hexl") and d[0]["range"]["start"]["line"] == 16, d)
d = request("textDocument/definition", td(MAIN, 13, 28))
check("#N -> #define", d and d[0]["uri"] == uri(MAIN) and d[0]["range"]["start"]["line"] == 3, d)
d = request("textDocument/definition", td(MAIN, 0, 12))
check("include path -> file", d and d[0]["uri"].endswith("lib/geom.hexl"), d)
d = request("textDocument/definition", td(MAIN, 1, 20))
check("std include path -> library", d and d[0]["uri"].endswith("library/os/vec.hexl"), d)

# ---- ホバー
h = request("textDocument/hover", td(MAIN, 13, 14))
check("hover shows signature + doc", h and "add<T>(a: T, b: T): T" in h["contents"]["value"] and "足し算" in h["contents"]["value"], h)
h = request("textDocument/hover", td(MAIN, 14, 5))
check("hover keyword", h and "loop" in h["contents"]["value"], h)
h = request("textDocument/hover", td(MAIN, 0, 0))
check("hover directive", h and "include" in h["contents"]["value"], h)

# ---- 補完
SC = PROJ / "scratch.hexl"
open_doc(SC, '#include "lib/geom.hexl"\nimpl Vec2 {\n    f(self: Vec2): int {\n        ret self.\n    }\n}\nmain() {\n    geom::\n    Vec2::\n    v.\n    #\n    r = geom::add<int>(1, \n}\n')
def labels(res):
    return {i["label"] for i in (res["items"] if isinstance(res, dict) else res)}
c = labels(request("textDocument/completion", td(SC, 7, 10)))
check("geom:: -> public members", {"add", "scale", "Vec2"} <= c, c)
c = labels(request("textDocument/completion", td(SC, 8, 10)))
check("Vec2:: -> methods", "len" in c, c)
c = labels(request("textDocument/completion", td(SC, 3, 17)))
check("self. -> members of enclosing type", {"x", "y", "len"} <= c, c)
c = labels(request("textDocument/completion", td(SC, 9, 6)))
check("v. -> members", {"x", "y"} <= c, c)
c = labels(request("textDocument/completion", td(SC, 10, 5)))
check("# -> directives", {"include", "define", "if", "endif", "windows"} <= c, c)
c = labels(request("textDocument/completion", td(SC, 6, 0)))
check("default -> keywords/snippets/defs", {"loop", "struct", "fn", "add", "Vec2", "geom"} <= c, c)
c = labels(request("textDocument/completion", td(SC, 0, 12)))
check("include path completion", "lib/" in c or "geom.hexl" in c or "main.hexl" in c, c)

# ---- シグネチャヘルプ
s = request("textDocument/signatureHelp", td(SC, 11, 26))
check("signature help", s and "add<T>" in s["signatures"][0]["label"] and s["activeParameter"] == 1, s)

# ---- 参照 / ハイライト
refs = request("textDocument/references", td(MAIN, 5, 6, context={"includeDeclaration": True}))
check("references across files", len(refs) >= 4 and any(r["uri"].endswith("geom.hexl") for r in refs) and any(r["uri"].endswith("main.hexl") for r in refs), len(refs))
hl = request("textDocument/documentHighlight", td(MAIN, 5, 6))
check("highlight", len(hl) >= 3, hl)

# ---- リネーム
check("prepareRename on local var is refused", request("textDocument/prepareRename", td(MAIN, 13, 4)) is None)
pr = request("textDocument/prepareRename", td(MAIN, 5, 6))
check("prepareRename on type", pr and pr["placeholder"] == "Vec2", pr)
ed = request("textDocument/rename", td(MAIN, 5, 6, newName="Point"))
check("rename edits both files", ed and any(k.endswith("geom.hexl") for k in ed["changes"]) and any(k.endswith("main.hexl") for k in ed["changes"]), ed and list(ed["changes"]))
check("rename rejects keyword", request("textDocument/rename", td(MAIN, 5, 6, newName="loop")) is None)

# ---- ワークスペースシンボル / 折りたたみ / セマンティックトークン
ws = request("workspace/symbol", {"query": "add"})
check("workspace symbol", any(s["name"] == "add" for s in ws), ws)
fr = request("textDocument/foldingRange", {"textDocument": {"uri": uri(MAIN)}})
check("folding ranges", len(fr) >= 3, fr)
st = request("textDocument/semanticTokens/full", {"textDocument": {"uri": uri(MAIN)}})
check("semantic tokens", len(st["data"]) > 0 and len(st["data"]) % 5 == 0, len(st["data"]))

# ---- 終了
request("shutdown", None)
notify("exit", None)
check("exit code 0", p.wait() == 0)
print("\n%s" % ("ALL PASSED" if not fails else f"{fails} FAILED"))
sys.exit(1 if fails else 0)
