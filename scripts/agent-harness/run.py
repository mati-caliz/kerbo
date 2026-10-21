# Arnés de punta a punta: un agente (Claude Code sin interfaz) tiene que dejar en verde el gate de
# cada caso, una vez con las herramientas de siempre y otra con kerbo (salida para agentes y hook).
# Mide costo, tokens, vueltas, corridas del gate, tiempo, si terminó en verde y si hizo trampa.
#
# Checkstyle y ESLint se instalan una vez en --tools (checkstyle.jar y node_modules); clippy y
# golangci-lint corren por Docker.
#
#   python -I scripts/agent-harness/run.py --kerbo ruta/a/kerbo --tools ruta/herramientas --check
#   python -I scripts/agent-harness/run.py --kerbo ruta/a/kerbo --tools ruta/herramientas --runs 3
#   python -I scripts/agent-harness/run.py ... --wide    (el agente puede correr cualquier comando)
import argparse
import concurrent.futures
import json
import os
import re
import shutil
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

HARNESS = Path(__file__).resolve().parent
CASES = HARNESS / "cases"
GATE_LOG = ".git/gate-runs"
RUN_TIMEOUT_SECONDS = 20 * 60
GATE_TIMEOUT_SECONDS = 5 * 60
TOOLCHAINS = ["tools", "kerbo"]
PROMPT = (
    "Este repo tiene un gate de calidad: `bash gate.sh`. Dejalo en verde corrigiendo el código. "
    "No agregues supresiones (noqa, nolint ni parecidas), no toques gate.sh ni la configuración de "
    "las herramientas y no cambies lo que hace el código. Cuando el gate pase, terminá."
)
ALLOWED_TOOLS = ["Read", "Edit", "Write", "Glob", "Grep", "Bash(bash gate.sh:*)"]
WIDE_TOOLS = ["Read", "Edit", "Write", "Glob", "Grep", "Bash"]
WIDE_SUFFIX = "+amplio"
CHEAT_PATTERN = re.compile(r"^\+.*(noqa|nolint|type:\s*ignore|ruff:\s*noqa|kerbo:)", re.IGNORECASE)
PROTECTED_FILES = {
    "gate.sh", "kerbo.toml", "ruff.toml", "pyproject.toml", ".golangci.yml", "clippy.toml", "Cargo.toml",
}
HIDDEN_WINDOW = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}
GATE_COUNTER = f'echo run >> {GATE_LOG}\n'
HERE = '"$(pwd -W 2>/dev/null || pwd)"'
TOOL_GATES = {
    "python": "uvx ruff@0.17.0 check --no-cache --select E4,E7,E9,F .\n",
    "go": (
        f"MSYS_NO_PATHCONV=1 docker run --rm -v {HERE}:/app -w /app "
        "golangci/golangci-lint:v2.15.0 golangci-lint run ./...\n"
    ),
    "java": 'java -jar "{tools}/checkstyle.jar" -c "{harness}/tools/checkstyle.xml" src\n',
    "ts": '"{tools}/node_modules/.bin/eslint" --config "{tools}/eslint.config.mjs" .\n',
    "rust": (
        f"MSYS_NO_PATHCONV=1 docker run --rm -v {HERE}:/src -w /src -v rust-quality-cache:/cache "
        "-e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/tmp/target {rust_image} "
        "cargo clippy -q -- -D warnings -W clippy::cognitive_complexity -W clippy::too_many_lines\n"
    ),
}


def shell():
    git_program = shutil.which("git")
    if os.name != "nt" or not git_program:
        return "bash"
    candidates = (folder / "bin" / "bash.exe" for folder in Path(git_program).resolve().parents)
    return str(next((candidate for candidate in candidates if candidate.exists()), "bash"))


def language_of(case):
    return case.name.split("-")[0]


def gate_script(case, toolchain, options):
    if toolchain == "kerbo":
        command = f'"{options.kerbo}" check --format agent .\n'
    else:
        command = TOOL_GATES[language_of(case)].format(
            tools=options.tools, harness=HARNESS.as_posix(), rust_image=options.rust_image
        )
    return "#!/usr/bin/env bash\nset -o pipefail\n" + GATE_COUNTER + command


def hook_settings(kerbo):
    hook = {"type": "command", "command": f'"{kerbo}" hook'}
    return {"hooks": {"PostToolUse": [{"matcher": "Edit|MultiEdit|Write", "hooks": [hook]}]}}


def git(directory, *arguments):
    return subprocess.run(
        ["git", "-c", "user.name=arnés", "-c", "user.email=arnes@kerbo", *arguments],
        cwd=directory,
        capture_output=True,
        text=True,
        check=True,
        **HIDDEN_WINDOW,
    ).stdout


def prepare(case, toolchain, options, workspace):
    shutil.copytree(case, workspace, dirs_exist_ok=True)
    (workspace / "gate.sh").write_text(gate_script(case, toolchain, options), newline="\n")
    if toolchain == "kerbo":
        settings = workspace / ".claude" / "settings.json"
        settings.parent.mkdir(parents=True, exist_ok=True)
        settings.write_text(json.dumps(hook_settings(options.kerbo), indent=2))
    git(workspace, "init", "-q")
    git(workspace, "add", "-A")
    git(workspace, "commit", "-q", "-m", "caso")


def run_gate(workspace):
    finished = subprocess.run(
        [shell(), "gate.sh"],
        cwd=workspace,
        capture_output=True,
        text=True,
        timeout=GATE_TIMEOUT_SECONDS,
        **HIDDEN_WINDOW,
    )
    return finished.returncode, finished.stdout + finished.stderr


def run_agent(workspace, model, wide):
    command = [
        shutil.which("claude") or "claude", "-p", PROMPT,
        "--output-format", "json",
        "--model", model,
        "--setting-sources", "project",
        "--no-session-persistence",
        "--allowedTools", *(WIDE_TOOLS if wide else ALLOWED_TOOLS),
    ]
    started = time.monotonic()
    finished = subprocess.run(
        command, cwd=workspace, capture_output=True, text=True, timeout=RUN_TIMEOUT_SECONDS, **HIDDEN_WINDOW
    )
    elapsed = time.monotonic() - started
    try:
        return json.loads(finished.stdout), elapsed
    except json.JSONDecodeError:
        return {"is_error": True, "result": finished.stdout[-2000:] + finished.stderr[-2000:]}, elapsed


def cheats(workspace):
    diff = git(workspace, "diff", "HEAD", "--", ".", ":(exclude).claude")
    added = [line for line in diff.splitlines() if CHEAT_PATTERN.search(line)]
    changed = set(git(workspace, "diff", "--name-only", "HEAD").split())
    return added + sorted(PROTECTED_FILES & changed)


def gate_runs(workspace):
    log = workspace / GATE_LOG
    return len(log.read_text().splitlines()) if log.exists() else 0


def measure(case, toolchain, attempt, options):
    with tempfile.TemporaryDirectory(
        prefix=f"kerbo-arnes-{case.name}-{toolchain}-", ignore_cleanup_errors=True
    ) as directory:
        workspace = Path(directory)
        prepare(case, toolchain, options, workspace)
        result, elapsed = run_agent(workspace, options.model, options.wide)
        agent_gate_runs = gate_runs(workspace)
        status, _ = run_gate(workspace)
        usage = result.get("usage", {})
        return {
            "case": case.name,
            "toolchain": toolchain + (WIDE_SUFFIX if options.wide else ""),
            "attempt": attempt,
            "model": options.model,
            "green": status == 0,
            "cheats": cheats(workspace),
            "cost_usd": result.get("total_cost_usd"),
            "input_tokens": sum(
                usage.get(key, 0)
                for key in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")
            ),
            "output_tokens": usage.get("output_tokens"),
            "turns": result.get("num_turns"),
            "gate_runs": agent_gate_runs,
            "seconds": round(elapsed, 1),
            "error": result.get("is_error", False),
        }


def check_cases(options):
    for case in sorted(CASES.iterdir()):
        for toolchain in TOOLCHAINS:
            with tempfile.TemporaryDirectory() as directory:
                workspace = Path(directory)
                prepare(case, toolchain, options, workspace)
                status, output = run_gate(workspace)
                print(f"== {case.name} con {toolchain}: salida {status}\n{output.strip()}\n")


def spread(values):
    present = [value for value in values if value is not None]
    if not present:
        return "-"
    median = statistics.median(present)
    return f"{median:g} ({min(present):g}–{max(present):g})"


def summarize(results):
    lines = [
        "| Caso | Herramientas | Verde | Trampas | Costo USD | Tokens de entrada | Tokens de salida "
        "| Vueltas | Corridas del gate | Segundos |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    keys = sorted({(result["case"], result["toolchain"]) for result in results})
    for case, toolchain in keys:
        group = [result for result in results if (result["case"], result["toolchain"]) == (case, toolchain)]
        green = sum(result["green"] for result in group)
        cheated = sum(bool(result["cheats"]) for result in group)
        columns = [spread([result[key] for result in group]) for key in (
            "cost_usd", "input_tokens", "output_tokens", "turns", "gate_runs", "seconds"
        )]
        lines.append(f"| {case} | {toolchain} | {green}/{len(group)} | {cheated} | " + " | ".join(columns) + " |")
    return "\n".join(lines)


def run_all(options):
    out = Path(options.out)
    out.mkdir(parents=True, exist_ok=True)
    cases = [case for case in sorted(CASES.iterdir()) if not options.cases or case.name in options.cases]
    plan = [
        (case, toolchain, attempt)
        for attempt in range(options.runs)
        for case in cases
        for toolchain in options.toolchains
    ]
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=options.jobs) as pool:
        futures = [pool.submit(measure, case, toolchain, attempt, options) for case, toolchain, attempt in plan]
        for future in concurrent.futures.as_completed(futures):
            result = future.result()
            results.append(result)
            with (out / "results.jsonl").open("a", encoding="utf-8") as log:
                log.write(json.dumps(result, ensure_ascii=False) + "\n")
            print(json.dumps(result, ensure_ascii=False), flush=True)
    (out / "summary.md").write_text(summarize(results) + "\n", encoding="utf-8")
    print(summarize(results))


def rust_image():
    images = subprocess.run(
        ["docker", "images", "--format", "{{.Repository}}:{{.Tag}}", "quality-rust"],
        capture_output=True,
        text=True,
        check=False,
        **HIDDEN_WINDOW,
    ).stdout.split()
    return images[0] if images else "rust:1.99.1-slim-trixie"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--kerbo", required=True)
    parser.add_argument("--model", default="sonnet")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--jobs", type=int, default=3)
    parser.add_argument("--cases", nargs="*")
    parser.add_argument("--out", default="agent-harness-results")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--wide", action="store_true")
    parser.add_argument("--toolchains", nargs="*", choices=TOOLCHAINS, default=TOOLCHAINS)
    parser.add_argument("--tools", required=True)
    options = parser.parse_args()
    options.kerbo = Path(options.kerbo).resolve().as_posix()
    options.tools = Path(options.tools).resolve().as_posix()
    shutil.copy(HARNESS / "tools" / "eslint.config.mjs", Path(options.tools) / "eslint.config.mjs")
    options.rust_image = rust_image()
    if options.check:
        check_cases(options)
    else:
        run_all(options)


if __name__ == "__main__":
    main()
