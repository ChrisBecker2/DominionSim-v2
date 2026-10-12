"""Generate crates/lab/src/recommended.json from the local wiki copy's
'Recommended Kingdoms_<Set>.json' pages ({{Kingdom|card|...|title=Name}} templates).
Only card names, kingdom titles and the colony/shelters flags are taken.
usage: python tools/gen_recommended.py <wiki pages dir> <output json>"""
import json, os, re, sys

pages, out = sys.argv[1], sys.argv[2]
SETS = ["Dominion", "Intrigue", "Seaside", "Prosperity", "Alchemy", "Cornucopia & Guilds", "Dark Ages", "Adventures", "Empires", "Nocturne", "Rising Sun", "Hinterlands"]
skip_keys = {"ally", "way1", "prophecy", "trait1", "traitfor1", "traitfor", "riverboat", "ferryman", "mouse", "appr_army", "bane", "project1", "project2"}
result = []
for s in SETS:
    f = os.path.join(pages, "Recommended Kingdoms_%s.json" % s)
    if not os.path.exists(f):
        continue
    text = json.load(open(f, encoding="utf-8"))["wikitext"]
    for m in re.finditer(r"\{\{Kingdom\|(.*?)\}\}", text, re.S):
        cards, named = [], {}
        for p in m.group(1).split("|"):
            p = p.strip()
            if "=" in p:
                k, v = p.split("=", 1)
                named[k.strip()] = v.strip()
            elif p:
                cards.append(p)
        if len(cards) != 10 or skip_keys & set(named):
            continue
        extras = [named[k] for k in ("event1", "event2", "landmark1", "landmark2") if k in named]
        result.append({
            # Titles may be wiki links ("[[First Game]]"): keep the shown text.
            "name": re.sub(r"\[\[(?:[^|\]]*\|)?([^\]]*)\]\]", r"\1", named.get("title", "?")),
            "set": "Base" if s == "Dominion" else s,
            "cards": cards,
            "extras": extras,
            "colony": named.get("colony") == "1",
            "shelters": named.get("shelters") == "1",
        })
json.dump(result, open(out, "w", encoding="utf-8", newline="\n"), indent=1, ensure_ascii=False)
print(len(result), "kingdoms")
