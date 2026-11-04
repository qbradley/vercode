#!/usr/bin/env python3
"""Prepare isolated codec tasks and grade their outputs with independent checks."""

import argparse
from contextlib import contextmanager
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import statistics
import time
import uuid


EVALS = Path(__file__).resolve().parents[1]
REPO = EVALS.parents[1]
SKILL = REPO / "vercode" / "agent-plugin" / "skills" / "custom-vercodable"
CASES = ("create", "update", "enums")
WIRE_REVISION = "global-root-frame-v3"
BUILD = REPO / "target" / f"custom-vercodable-evals-build-{WIRE_REVISION}"
LEGACY_WORKSPACE = REPO / "target" / "skill-evals" / "custom-vercodable-workspace"
EXTERNAL_WORKSPACE = REPO / "target" / "skill-evals" / "custom-vercodable-append-workspace"
MUTATIONS = {
    "create": (
        ("root", "Sample", "reorder", "id", "point"),
        ("embedded", "Point", "reorder", "x", "y"),
        ("behind_vec", "Entry", "reorder", "point", "weight"),
        ("array", "Sample", "array", "bytes", None),
        ("container", "Sample", "vec", "samples", ("u16", "i16")),
        ("entry_argument", "Sample", "vec", "entries", ("Entry", "Point")),
        ("option", "Sample", "option", "flags", ("bool", "u8")),
        ("ticket_argument", "Sample", "option", "ticket", ("NonZeroU32", "u32")),
    ),
    "update": (
        ("root", "Packet", "reorder", "child", "marker"),
        ("embedded", "Child", "reorder", "low", "high"),
        ("container", "Packet", "vec", "values", ("u16", "i16")),
    ),
    "enums": (
        ("root", "Frame", "reorder", "mode", "strict"),
        ("embedded", "Block", "reorder", "a", "b"),
        ("variant", "Mode", "variant", "Unknown", "Idle"),
        ("new_subtree", "Extension", "reorder", "modes", "checksum"),
        ("container", "Frame", "vec", "modes", ("Mode", "Strict")),
        ("option", "Frame", "option", "extension", ("Extension", "Block")),
        ("new_container", "Extension", "vec", "modes", ("Mode", "Strict")),
    ),
}


def refuse_legacy_write(path):
    historical = (LEGACY_WORKSPACE, EXTERNAL_WORKSPACE,
                  EVALS / "legacy-framed-results.json", EVALS / "legacy-framed-results.md",
                  EVALS / "historical-external-results.json", EVALS / "historical-external-results.md")
    if any(path.resolve().is_relative_to(root.resolve()) for root in historical):
        raise SystemExit("Historical evaluation artifacts are read-only; use a fresh workspace.")
    # A copied historical run is still historical, even outside the known workspaces.
    for parent in (path, *path.parents):
        for name in ("eval_metadata.json", "grading.json", "benchmark.json"):
            metadata = parent / name
            if metadata.is_file():
                data = json.loads(metadata.read_text())
                revision = data.get("wire_revision", data.get("metadata", {}).get("wire_revision"))
                if revision is not None and revision != WIRE_REVISION:
                    raise SystemExit(f"Historical wire revision {revision} at {metadata} is read-only.")


def require_revision(data, label):
    if data.get("wire_revision") != WIRE_REVISION:
        raise SystemExit(f"{label} must identify wire_revision {WIRE_REVISION}; do not regrade historical trials.")


def require_candidate(case, output):
    refuse_legacy_write(output)
    metadata = output.parent / "eval_metadata.json"
    if not metadata.is_file():
        raise SystemExit("Missing eval_metadata.json; prepare a fresh candidate for this wire revision.")
    data = json.loads(metadata.read_text())
    require_revision(data, str(metadata))
    if data.get("eval_name") != case:
        raise SystemExit(f"Expected {case} candidate metadata at {metadata}.")


@contextmanager
def isolated_project(output, label):
    # Explicit project-local probe paths, never the system temporary directory.
    root = BUILD / "probes" / f"{label}-{uuid.uuid4().hex}"
    project = root / "project"
    try:
        shutil.copytree(output, project, ignore=shutil.ignore_patterns("target", ".git", "tests"))
        yield project
    finally:
        if root.exists():
            shutil.rmtree(root)


def save_json(path, value):
    refuse_legacy_write(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def command(args, cwd, log):
    refuse_legacy_write(log)
    # Identically named candidate crates need separate artifacts during parallel trials.
    run_key = hashlib.sha256(str(log.parent.parent.resolve()).encode()).hexdigest()[:16]
    env = dict(os.environ, CARGO_TARGET_DIR=str(BUILD / run_key), CARGO_TERM_COLOR="never")
    started = time.monotonic()
    try:
        result = subprocess.run(
            args, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, timeout=240, check=False,
        )
        code, output = result.returncode, result.stdout
    except subprocess.TimeoutExpired as error:
        output = error.stdout or b""
        if isinstance(output, bytes):
            output = output.decode(errors="replace")
        code, output = 124, output + "\nEvaluation command timed out.\n"
    log.parent.mkdir(parents=True, exist_ok=True)
    log.write_text(output)
    return code, output, time.monotonic() - started


def prepare(case, output):
    refuse_legacy_write(output)
    if output.exists():
        raise SystemExit(f"Refusing to overwrite existing output: {output}")
    (output / "src").mkdir(parents=True)
    manifest = (EVALS / "fixtures" / "Cargo.toml.in").read_text()
    # JSON string escaping is valid for this TOML basic-string value.
    manifest = manifest.replace('"@VERCODE_PATH@"', json.dumps(str(REPO / "vercode")))
    (output / "Cargo.toml").write_text(manifest)
    source = (EVALS / "fixtures" / f"{case}.rs").read_text()
    (output / "src" / "lib.rs").write_text(source)
    if case == "update":
        (output / "historical.rs").write_text(source)
    catalog = json.loads((EVALS / "evals.json").read_text())
    entry = next(item for item in catalog["evals"] if item["name"] == case)
    metadata = {
        "wire_revision": WIRE_REVISION,
        "eval_id": entry["id"], "eval_name": case, "prompt": entry["prompt"],
        "assertions": entry["expectations"],
    }
    save_json(output.parent / "eval_metadata.json", metadata)
    if output.parent.name.startswith("run-"):
        save_json(output.parents[2] / "eval_metadata.json", metadata)
    print(entry["prompt"])


def prepare_decisions(output):
    refuse_legacy_write(output)
    if output.exists():
        raise SystemExit(f"Refusing to overwrite existing output: {output}")
    output.mkdir(parents=True)
    entries = json.loads((EVALS / "evals.json").read_text())["evals"][3:]
    prompts = [{"id": item["id"], "name": item["name"], "prompt": item["prompt"]}
               for item in entries]
    save_json(output.parent / "prompts.json", prompts)
    metadata = {
        "wire_revision": WIRE_REVISION,
        "eval_id": 4,
        "eval_name": "decisions-batch",
        "prompt": "Answer these design requests separately:\n\n" + "\n\n".join(
            f'{item["id"]}. {item["prompt"]}' for item in prompts
        ),
        "assertions": [expectation for item in entries for expectation in item["expectations"]],
        "batch_case_ids": [item["id"] for item in entries],
    }
    save_json(output.parent / "eval_metadata.json", metadata)
    if output.parent.name.startswith("run-"):
        save_json(output.parents[2] / "eval_metadata.json", metadata)


def owner_span(source, owner):
    """Find a named definition's body, ignoring braces in comments and strings."""
    masked = re.sub(
        r'//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"',
        lambda match: "".join("\n" if c == "\n" else " " for c in match.group()),
        source,
    )
    match = re.search(rf"\b(?:struct|enum)\s+{re.escape(owner)}\s*(?:<[^{{;]*>)?\s*\{{", masked)
    if not match:
        return None
    start = match.end()
    depth = 1
    for offset in range(start, len(masked)):
        if masked[offset] == "{":
            depth += 1
        elif masked[offset] == "}":
            depth -= 1
            if not depth:
                return start, offset
    return None


def mutate(source, mutation):
    _, owner, operation, first, second = mutation
    span = owner_span(source, owner)
    if span is None:
        return source, 0
    start, end = span
    body = source[start:end]
    if operation == "array":
        replacement, count = re.subn(
            rf"(\bpub\s+{first}:\s*\[u8;\s*)8(\s*\])", r"\g<1>9\2", body, count=1,
        )
    elif operation in ("vec", "option"):
        container = "Vec" if operation == "vec" else "Option"
        old, new = second
        replacement, count = re.subn(
            rf"(\bpub\s+{first}:\s*{container}\s*<\s*){old}(\s*>)",
            rf"\g<1>{new}\2", body, count=1,
        )
    else:
        if operation == "variant":
            first_match = re.search(rf"\b{first}\s*,", body)
            second_match = re.search(rf"\b{second}\s*,", body)
        else:
            first_match = re.search(rf"\bpub\s+{first}:\s*[^,\n]+,", body)
            second_match = re.search(rf"\bpub\s+{second}:\s*[^,\n]+,", body)
        replacement, count = swap_matches(body, first_match, second_match)
    return source[:start] + replacement + source[end:], count


def swap_matches(source, first_match, second_match):
    if first_match is None or second_match is None:
        return source, 0
    if first_match.start() > second_match.start():
        first_match, second_match = second_match, first_match
    result = (
        source[:first_match.start()] + second_match.group()
        + source[first_match.end():second_match.start()] + first_match.group()
        + source[second_match.end():]
    )
    return result, 1


def guard_failed(status, diagnostic):
    return bool(
        status != 0 and "E0080" in diagnostic
        and re.search(r"(?i)(custom-vercodable|ask an? (?:llm|ai)|type tree changed)", diagnostic)
        and re.search(r"(?i)(update|regenerate)", diagnostic)
    )


def inject_contract(case, output):
    require_candidate(case, output)
    tests = output / "tests"
    tests.mkdir(exist_ok=True)
    contract = EVALS / "checks" / f"{case}.rs"
    common = EVALS / "checks" / "common.rs"
    # Remove only exact legacy grader copies, never candidate-authored tests.
    legacy_contract = contract.read_text().replace("skill_eval_support/common.rs", "common.rs")
    for path, expected in ((tests / "contract.rs", legacy_contract),
                           (tests / "common.rs", common.read_text())):
        if path.exists() and path.read_text() == expected:
            path.unlink()
    (tests / "skill_eval_support").mkdir(exist_ok=True)
    shutil.copyfile(contract, tests / "skill_eval_contract.rs")
    shutil.copyfile(common, tests / "skill_eval_support" / "common.rs")
    return re.findall(r"#\[test\]\s*fn (\w+)", contract.read_text())


def grade(case, output, run_dir):
    refuse_legacy_write(output)
    refuse_legacy_write(run_dir)
    logs = run_dir / "logs"
    expectations = []

    def record(text, passed, evidence):
        expectations.append({"text": text, "passed": bool(passed), "evidence": evidence})

    test_names = inject_contract(case, output)
    code, stdout, elapsed = command(
        ["cargo", "test", "--test", "skill_eval_contract", "--", "--test-threads=1"],
        output, logs / "contract.log",
    )
    for name in test_names:
        passed = re.search(rf"^test {re.escape(name)} \.\.\. ok$", stdout, re.MULTILINE)
        record(name, passed is not None, f"logs/contract.log (cargo exit {code})")

    # Format and lint candidate code, not the grader's injected sources.
    source_files = sorted((output / "src").rglob("*.rs"))
    code, _, _ = command(
        ["rustfmt", "--edition", "2024", "--check", *map(str, source_files)],
        output, logs / "format.log",
    )
    record("candidate_formatting", code == 0, "logs/format.log")
    code, _, _ = command(
        ["cargo", "clippy", "--lib", "--no-deps", "--", "-D", "warnings"],
        output, logs / "clippy.log",
    )
    record("candidate_lints", code == 0, "logs/clippy.log")

    code, _, _ = command(["cargo", "check", "--lib"], output, logs / "unchanged.log")
    record("unchanged_production_build", code == 0, "logs/unchanged.log")
    source = "\n".join(path.read_text() for path in source_files)
    # The static review viewer lists only files immediately inside outputs/.
    (output / "implementation.rs").write_text(source)
    children = {"create": ("Point", "Entry"), "update": ("Child",),
                "enums": ("Block", "Extension")}[case]
    for child in children:
        with isolated_project(output, f"no-codec-{case}-{child}") as project:
            (project / "examples").mkdir(exist_ok=True)
            (project / "examples" / "eval_no_nested_codec.rs").write_text(
                "fn requires_codec<T: vercode::VerCodable>() {}\n"
                f"fn main() {{ requires_codec::<custom_vercodable_eval::{child}>(); }}\n"
            )
            logfile = f"no-nested-codec-{child}.log"
            status, diagnostic, _ = command(
                ["cargo", "check", "--example", "eval_no_nested_codec"], project, logs / logfile,
            )
            record(
                f"nested_type_{child}_remains_without_codec",
                status != 0 and "E0277" in diagnostic and child in diagnostic
                and "VerCodable" in diagnostic,
                f"logs/{logfile}: trait bound applied to the actual public child type.",
            )
    if case == "update":
        frozen = (EVALS / "fixtures" / "update.rs").read_bytes()
        actual = output / "historical.rs"
        record("historical_implementation_unchanged",
               actual.exists() and actual.read_bytes() == frozen,
               "Byte-for-byte comparison with skill-evals/custom-vercodable/fixtures/update.rs.")

    for mutation in MUTATIONS[case]:
        kind = mutation[0]
        with isolated_project(output, f"guard-{case}-{kind}") as project:
            changes = 0
            for path in sorted((project / "src").rglob("*.rs")):
                replacement, count = mutate(path.read_text(), mutation)
                if count:
                    path.write_text(replacement)
                    changes += count
                    break
            if changes:
                status, diagnostic, _ = command(
                    ["cargo", "check", "--lib"], project, logs / f"guard-{kind}.log",
                )
                failed_guard = guard_failed(status, diagnostic)
            else:
                failed_guard = False
            record(f"compile_guard_{kind}_mutation", failed_guard,
                   f"logs/guard-{kind}.log" if changes else "Mutation target not found.")

    passed = sum(item["passed"] for item in expectations)
    grading = {
        "wire_revision": WIRE_REVISION,
        "evaluation_kind": json.loads((output.parent / "eval_metadata.json").read_text()).get(
            "evaluation_kind", "model-trial"),
        "expectations": expectations,
        "summary": {
            "passed": passed, "failed": len(expectations) - passed,
            "total": len(expectations), "pass_rate": passed / len(expectations),
        },
        "timing": {"contract_duration_seconds": elapsed},
        "eval_feedback": {
            "overall": "Executable checks do not grade documentation, uniformity, actual speed, "
                       "cross-target execution, or all possible malformed inputs. Review those separately."
        },
        "source_sha256": {
            str(path.relative_to(output)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in source_files
        },
    }
    save_json(run_dir / "grading.json", grading)
    print(json.dumps(grading["summary"]))
    return passed == len(expectations)


PERFORMANCE_POLICY = (
    "For EACH current-version (v2) workload with 32, 512, and 4096 Entry elements: "
    "custom encoding must be strictly smaller; write, read_owned, and size must EACH "
    "have a lower custom median and at least 7 custom wins in 9 paired samples. "
    "Base v0 workloads are reported separately and do not determine acceptance."
)


def summarize_performance(samples):
    expected = {(elements, version) for elements in (32, 512, 4096) for version in (0, 2)}
    if len(samples) != 6 or {(s["elements"], s["version"]) for s in samples} != expected:
        raise ValueError("Expected six distinct workloads: 32/512/4096 entries at versions 0/2")
    for workload in samples:
        if workload["samples"] != 9:
            raise ValueError("Expected nine paired samples per workload")
        custom_size, derived_size = workload["custom_bytes"], workload["derive_bytes"]
        if not (isinstance(custom_size, int) and isinstance(derived_size, int)
                and custom_size > 0 and derived_size > 0):
            raise ValueError("Encoded sizes must be positive integers")
        workload["savings_bytes"] = derived_size - custom_size
        workload["size_smaller"] = custom_size < derived_size
        comparison = workload["comparison"] = {}
        for method in ("write", "read_owned", "size"):
            custom = workload["custom_ns"][method]
            derived = workload["derive_ns"][method]
            if any(len(values) != 9 or any(
                not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0
                for value in values
            ) for values in (custom, derived)):
                raise ValueError("Timing arrays must contain nine positive finite paired samples")
            custom_median, derive_median = statistics.median(custom), statistics.median(derived)
            wins = sum(left < right for left, right in zip(custom, derived))
            passed = custom_median < derive_median and wins >= 7
            comparison[method] = {
                "custom_median_ns": custom_median, "derive_median_ns": derive_median,
                "custom_min_ns": min(custom), "custom_max_ns": max(custom),
                "derive_min_ns": min(derived), "derive_max_ns": max(derived),
                "paired_custom_wins": wins, "paired_samples": 9,
                "derive_over_custom_median": derive_median / custom_median,
                "custom_faster_accepted": passed,
                "verdict": "faster_by_policy" if passed else (
                    "slower_or_equal_median" if custom_median >= derive_median
                    else "inconclusive_paired_wins"
                ),
            }
        workload["performance_accepted"] = workload["size_smaller"] and all(
            item["custom_faster_accepted"] for item in comparison.values()
        )
    return {
        "passed": all(item["performance_accepted"] for item in samples if item["version"] == 2),
        "policy": PERFORMANCE_POLICY,
        "scope": "Observed current-version workloads only; not a universal speedup claim.",
    }


def benchmark(output, run_dir, confirm_idle=False):
    refuse_legacy_write(output)
    refuse_legacy_write(run_dir)
    if not confirm_idle:
        raise SystemExit(
            "Stop model/evaluation jobs first, then pass --confirm-no-evaluation-jobs. "
            "Do not run wall-clock benchmarks concurrently with model evaluations."
        )
    report = {
        "wire_revision": WIRE_REVISION,
        "case": "create", "methods": ["write", "read_owned", "size"],
        "statistics": ["paired_samples", "min", "median", "max"],
        "performance_acceptance": {"passed": False, "policy": PERFORMANCE_POLICY},
        "limitations": [
            "Shared-host wall-clock measurements are not statistical proof; repeat on an idle controlled host.",
            "No allocation counts, code-size measurement, cross-target execution, or confidence intervals.",
            "Base and current versions; owned read includes allocation/drop, write reuses buffers.",
            "Setup and semantic roundtrip validation are outside timing; pair order alternates each sample.",
            "Size savings and speed acceptance are separate; no universal speedup claim.",
        ],
    }
    tests = inject_contract("create", output)
    status, stdout, _ = command(
        ["cargo", "test", "--test", "skill_eval_contract", "--", "--test-threads=1"],
        output, run_dir / "logs" / "performance-behavior.log",
    )
    behavior_passed = status == 0 and all(
        re.search(rf"^test {re.escape(name)} \.\.\. ok$", stdout, re.MULTILINE)
        for name in tests
    )
    report["behavior_gate_passed"] = behavior_passed
    if not behavior_passed:
        report["performance_acceptance"]["reason"] = "Behavior checks failed; timing was not run."
        save_json(run_dir / "performance.json", report)
        print(json.dumps(report["performance_acceptance"]))
        return False

    examples = output / "examples"
    examples.mkdir(exist_ok=True)
    shutil.copyfile(EVALS / "checks" / "performance.rs", examples / "eval_performance.rs")
    status, stdout, _ = command(
        ["cargo", "run", "--release", "--example", "eval_performance"],
        output, run_dir / "logs" / "performance.log",
    )
    if status:
        report["performance_acceptance"]["reason"] = "Benchmark command failed; read logs/performance.log."
        save_json(run_dir / "performance.json", report)
        return False
    try:
        samples = [json.loads(line) for line in stdout.splitlines() if line.startswith('{"elements"')]
        report["performance_acceptance"] = summarize_performance(samples)
        report["workloads"] = samples
    except (ValueError, KeyError, TypeError) as error:
        report["performance_acceptance"]["reason"] = f"Invalid benchmark output: {error}"
        save_json(run_dir / "performance.json", report)
        return False
    _, compiler, _ = command(["rustc", "-vV"], output, run_dir / "logs" / "compiler.log")
    report["compiler"] = compiler.strip()
    report["source_sha256"] = {
        str(path.relative_to(output)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted((output / "src").rglob("*.rs"))
    }
    save_json(run_dir / "performance.json", report)
    print(json.dumps(report["performance_acceptance"]))
    return report["performance_acceptance"]["passed"]


def review_metadata(iteration, model, skill_path=SKILL):
    """Replace aggregator placeholders; never present missing telemetry as zero."""
    refuse_legacy_write(iteration)
    path = iteration / "benchmark.json"
    data = json.loads(path.read_text())
    for grade_path in iteration.glob("eval-*/*/run-*/grading.json"):
        grade_data = json.loads(grade_path.read_text())
        require_revision(grade_data, str(grade_path))
        if grade_data.get("evaluation_kind") == "harness-smoke":
            raise SystemExit("Harness smoke checks cannot become model evaluation results.")
    counts = {}
    for run in data["runs"]:
        key = (run["eval_id"], run["configuration"])
        counts[key] = counts.get(key, 0) + 1
    data["metadata"].update({
        "wire_revision": WIRE_REVISION,
        "skill_path": str(skill_path),
        "skill_files_sha256": {
            str(path.relative_to(skill_path)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in [skill_path / "SKILL.md", *sorted((skill_path / "references").glob("*.md"))]
        },
        "executor_model": model,
        "analyzer_model": model,
        "runs_per_configuration": max(counts.values(), default=0),
        "resource_metrics": "Unavailable: token counts and precise executor timing were not reported.",
    })
    for run in data["runs"]:
        for key in ("time_seconds", "tokens", "tool_calls", "errors"):
            run["result"].pop(key, None)
    for summary in data.get("run_summary", {}).values():
        if isinstance(summary, dict):
            for key in ("time_seconds", "tokens"):
                summary.pop(key, None)
    for key in ("time_seconds", "tokens"):
        data.get("delta", {}).pop(key, None)
    notes = data.setdefault("notes", [])
    notes.append(
        "One fresh agent per code case and configuration. Decision cases, when present, "
        "share one batch context. Scores do not estimate general reliability. "
        "Resource metrics are omitted because exact telemetry was unavailable."
    )
    analysis = iteration / "analysis.json"
    if analysis.exists():
        notes.extend(json.loads(analysis.read_text()))
    data["notes"] = list(dict.fromkeys(notes))
    save_json(path, data)
    # Replace the upstream Markdown summary that otherwise reports missing metrics as zero.
    lines = ["# Evaluation summary", "", "| Configuration | Passed | Total |",
             "|---|---:|---:|"]
    for configuration in ("with_skill", "without_skill"):
        results = [run["result"] for run in data["runs"] if run["configuration"] == configuration]
        lines.append(
            f"| {configuration} | {sum(item['passed'] for item in results)} "
            f"| {sum(item['total'] for item in results)} |"
        )
    lines.extend(["", "Exact executor duration and token counts are unavailable.", ""])
    (iteration / "benchmark.md").write_text("\n".join(lines))


def record_results(workspace, destination):
    refuse_legacy_write(destination)
    results = {"skill_name": "custom-vercodable", "wire_revision": WIRE_REVISION, "iterations": []}
    for iteration in sorted(workspace.glob("iteration-*")):
        benchmark_data = json.loads((iteration / "benchmark.json").read_text())
        require_revision(benchmark_data["metadata"], str(iteration / "benchmark.json"))
        entry = {"iteration": iteration.name, "metadata": benchmark_data["metadata"], "runs": []}
        for path in sorted(iteration.glob("eval-*/*/run-*/grading.json")):
            parts = path.relative_to(iteration).parts
            grade_data = json.loads(path.read_text())
            require_revision(grade_data, str(path))
            if grade_data.get("evaluation_kind") == "harness-smoke":
                raise SystemExit("Harness smoke checks cannot become model evaluation results.")
            run = {
                "case": parts[0].removeprefix("eval-"),
                "configuration": parts[1],
                "run": parts[2],
                "summary": grade_data["summary"],
                "expectations": grade_data["expectations"],
                "source_sha256": grade_data.get("source_sha256", {}),
                "artifact_path": str(path.parent.relative_to(REPO)),
            }
            performance = path.parent / "performance.json"
            if performance.exists():
                run["performance"] = json.loads(performance.read_text())
                require_revision(run["performance"], str(performance))
            previous_performance = sorted(path.parent.glob("performance-*.json"))
            if previous_performance:
                run["superseded_performance"] = {
                    item.name: json.loads(item.read_text()) for item in previous_performance
                }
                for name, data in run["superseded_performance"].items():
                    require_revision(data, str(path.parent / name))
            answers = path.parent / "outputs" / "answers.json"
            if answers.exists():
                run["source_sha256"]["outputs/answers.json"] = hashlib.sha256(answers.read_bytes()).hexdigest()
            entry["runs"].append(run)
        results["iterations"].append(entry)
    save_json(destination, results)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    prep = subparsers.add_parser("prepare")
    prep.add_argument("case", choices=CASES)
    prep.add_argument("output", type=Path)
    decisions = subparsers.add_parser("prepare-decisions")
    decisions.add_argument("output", type=Path)
    check = subparsers.add_parser("grade")
    check.add_argument("case", choices=CASES)
    check.add_argument("output", type=Path)
    check.add_argument("--run-dir", type=Path)
    bench = subparsers.add_parser("benchmark-create")
    bench.add_argument("output", type=Path)
    bench.add_argument("--run-dir", type=Path)
    bench.add_argument("--confirm-no-evaluation-jobs", action="store_true",
                       help="Confirm model/evaluation jobs have stopped before wall-clock timing")
    review = subparsers.add_parser("review-metadata")
    review.add_argument("output", type=Path, help="Iteration directory containing benchmark.json")
    review.add_argument("--model", required=True, help="Actual executor and grader model")
    review.add_argument("--skill-path", type=Path, default=SKILL, help="Exact evaluated skill snapshot")
    records = subparsers.add_parser("record-results")
    records.add_argument("output", type=Path, help="Workspace containing iteration directories")
    records.add_argument("destination", type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if args.action == "prepare":
        prepare(args.case, output)
    elif args.action == "prepare-decisions":
        prepare_decisions(output)
    elif args.action == "benchmark-create":
        run_dir = args.run_dir.resolve() if args.run_dir else output.parent
        raise SystemExit(0 if benchmark(output, run_dir, args.confirm_no_evaluation_jobs) else 1)
    elif args.action == "review-metadata":
        review_metadata(output, args.model, args.skill_path.resolve())
    elif args.action == "record-results":
        record_results(output, args.destination.resolve())
    else:
        run_dir = args.run_dir.resolve() if args.run_dir else output.parent
        raise SystemExit(0 if grade(args.case, output, run_dir) else 1)


if __name__ == "__main__":
    main()
