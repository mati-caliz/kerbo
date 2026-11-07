"""Lo que falta en out/kerbo.json contra out/shellcheck.json, agrupado por archivo, y si ese archivo tiene un
`.shellcheckrc` en su repo o un error de parseo. Sirve para separar lo que kerbo silencia a propósito de lo que
de verdad se pierde."""
import collections
import json
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
corpus = os.path.join(here, "..", "..", "..", ".parity", "shell-corpus")
families = open(os.path.join(here, "..", "..", "..", "src", "rules", "families", "shell.rs"), encoding="utf-8").read()
known = set(re.findall(r'"(sc\d+)"', families))
out = os.path.join(here, "out")
kerbo = json.load(open(os.path.join(out, "kerbo.json"), encoding="utf-8"))
analyzed = {finding["path"].removeprefix("./") for finding in kerbo}
reported = {(finding["rule"], finding["path"].removeprefix("./"), finding["line"]) for finding in kerbo}
expected = collections.Counter()
missing = collections.Counter()
for comment in json.load(open(os.path.join(out, "shellcheck.json"), encoding="utf-8")):
    rule = f'sc{comment["code"]}'
    if rule in known:
        expected[comment["file"]] += 1
        if (rule, comment["file"], comment["line"]) not in reported:
            missing[comment["file"]] += 1
print("total esperados", sum(expected.values()), "faltan", sum(missing.values()), "archivos", len(missing))
by_repo = collections.Counter()
for path, count in missing.items():
    by_repo[path.split("/")[0]] += count
print(by_repo.most_common())
rc_files = {repo: os.path.exists(os.path.join(corpus, repo, ".shellcheckrc")) for repo in by_repo}
print("con .shellcheckrc:", [repo for repo, present in rc_files.items() if present])
limit = int(sys.argv[1]) if len(sys.argv) > 1 else 30
for path, count in missing.most_common(limit):
    print(count, "de", expected[path], path, "" if path in analyzed else "(sin avisos en kerbo)")
