#!/usr/bin/env python3
"""Adapt copies of archived bodies for HARNESS smoke checks, never model evaluation."""

import argparse
from contextlib import redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import shutil

import run_evals


PREFIX = """
impl VerCodable for ROOT {
    const MAX_VERSION: u32 = VERSION;

    fn write_version(&self, version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        let total = self.size_version(version)?;
        let output = buf.get_mut(..total).ok_or(InvalidEncoding)?;
        output[..4].copy_from_slice(&(total as u32).to_le_bytes());
        self.write_body(version, &mut output[4..])?;
        Ok(total)
    }

    fn read_version(version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        let length = u32::from_le_bytes(buf.get(..4).ok_or(InvalidEncoding)?
            .try_into().map_err(|_| InvalidEncoding)?);
        let total = usize::try_from(length).map_err(|_| InvalidEncoding)?;
        let body = buf.get(4..total).ok_or(InvalidEncoding)?;
        let (value, _) = Self::read_body(version, body)?;
        Ok((value, total))
    }

    fn size_version(&self, version: u32) -> Result<usize, InvalidEncoding> {
        let total = self.size_body(version)?.checked_add(4).ok_or(InvalidEncoding)?;
        u32::try_from(total).map_err(|_| InvalidEncoding)?;
        Ok(total)
    }
}
"""

ADOPTION = """
impl VerCodable for Packet {
    const MAX_VERSION: u32 = 3;

    fn write_version(&self, version: u32, buf: &mut [u8]) -> Result<usize, InvalidEncoding> {
        let total = self.size_version(version)?;
        let output = buf.get_mut(..total).ok_or(InvalidEncoding)?;
        let mut body = vec![0; total - 4];
        self.write_body(version, &mut body)?;
        let base = self.size_body(0)?;
        output[..base].copy_from_slice(&body[..base]);
        output[base..base + 4].copy_from_slice(&((body.len() - base) as u32).to_le_bytes());
        output[base + 4..].copy_from_slice(&body[base..]);
        Ok(total)
    }

    fn read_version(version: u32, buf: &[u8]) -> Result<(Self, usize), InvalidEncoding> {
        let count = u64::from_le_bytes(buf.get(8..16).ok_or(InvalidEncoding)?
            .try_into().map_err(|_| InvalidEncoding)?);
        let base = usize::try_from(count).map_err(|_| InvalidEncoding)?
            .checked_mul(2).and_then(|n| n.checked_add(16)).ok_or(InvalidEncoding)?;
        let start = base.checked_add(4).ok_or(InvalidEncoding)?;
        let extension = u32::from_le_bytes(buf.get(base..start).ok_or(InvalidEncoding)?
            .try_into().map_err(|_| InvalidEncoding)?);
        let total = start.checked_add(usize::try_from(extension).map_err(|_| InvalidEncoding)?)
            .ok_or(InvalidEncoding)?;
        let extensions = buf.get(start..total).ok_or(InvalidEncoding)?;
        let mut body = buf[..base].to_vec();
        body.extend(extensions);
        let (value, _) = Self::read_body(version, &body)?;
        Ok((value, total))
    }

    fn size_version(&self, version: u32) -> Result<usize, InvalidEncoding> {
        let size = self.size_body(version)?;
        u32::try_from(size - self.size_body(0)?).map_err(|_| InvalidEncoding)?;
        size.checked_add(4).ok_or(InvalidEncoding)
    }
}
"""


def adapt(source, case):
    root, version = {"create": ("Sample", 2), "update": ("Packet", 3), "enums": ("Frame", 3)}[case]
    original = f"impl VerCodable for {root} {{\n    const MAX_VERSION: u32 = {version};"
    if source.count(original) != 1:
        raise ValueError(f"{case}: expected one archived root implementation")
    source = source.replace(original, f"impl {root} {{")
    for method in ("read", "write", "size"):
        source = source.replace(f"fn {method}_version(", f"fn {method}_body(")
    wrapper = ADOPTION if case == "update" else PREFIX.replace("ROOT", root).replace("VERSION;", f"{version};")
    # Archived comments describe the old raw body, not the smoke wrapper.
    return "// HARNESS SMOKE ONLY: copied historical body plus an envelope, NOT a model trial.\n" + source + wrapper


def smoke(historical_workspace, destination):
    if destination.exists():
        raise SystemExit(f"Refusing to overwrite smoke artifacts: {destination}")
    run_evals.refuse_legacy_write(destination)
    sources = {}
    for case in run_evals.CASES:
        path = historical_workspace / f"iteration-1/eval-{case}/with_skill/run-1/outputs/src/lib.rs"
        sources[case] = (path, path.read_text())
    destination.mkdir(parents=True)
    (destination / "HARNESS_SMOKE.txt").write_text(
        "HARNESS SANITY CHECKS ONLY. No fresh model skill evaluation or benchmark.\n"
        "Sources are adapted copies; archived scored sources are unchanged.\n"
        "Adoption wrapper copies buffers and is NOT a performance candidate.\n"
    )
    results = {}
    for case, (source_path, source) in sources.items():
        run = destination / case
        output = run / "outputs"
        with redirect_stdout(io.StringIO()):
            run_evals.prepare(case, output)
        metadata_path = run / "eval_metadata.json"
        metadata = json.loads(metadata_path.read_text())
        metadata.update({
            "evaluation_kind": "harness-smoke",
            "historical_source": str(source_path),
            "historical_source_sha256": hashlib.sha256(source.encode()).hexdigest(),
        })
        run_evals.save_json(metadata_path, metadata)
        (output / "src/lib.rs").write_text(adapt(source, case))
        code, _, _ = run_evals.command(
            ["rustfmt", "--edition", "2024", str(output / "src/lib.rs")],
            output, run / "logs/smoke-format.log",
        )
        if code:
            raise SystemExit(f"Smoke formatting failed: {run}")
        passed = run_evals.grade(case, output, run)
        if case == "create":
            (output / "examples").mkdir(exist_ok=True)
            shutil.copyfile(run_evals.EVALS / "checks/performance.rs", output / "examples/eval_performance.rs")
            code, _, _ = run_evals.command(
                ["cargo", "check", "--example", "eval_performance"],
                output, run / "logs/benchmark-compile-only.log",
            )
            passed = passed and code == 0
        results[case] = passed
        if not passed:
            raise SystemExit(f"Harness smoke failed: inspect {run}/logs")
        # A wrong consumed length must fail even when ordinary round trips succeed.
        with run_evals.isolated_project(output, f"smoke-negative-{case}") as probe:
            run_evals.save_json(probe.parent / "eval_metadata.json", metadata)
            run_evals.inject_contract(case, probe)
            library = probe / "src/lib.rs"
            correct = library.read_text()
            invalid_consumption = {
                "buf-len": "Ok((value, buf.len()))",
                "decoded-size": "let consumed = value.size_version(version)?; Ok((value, consumed))",
                "known-size": "let consumed = value.size_version(Self::MAX_VERSION)?; Ok((value, consumed))",
            }
            for name, replacement in invalid_consumption.items():
                library.write_text(correct.replace("Ok((value, total))", replacement))
                code, text, _ = run_evals.command(
                    ["cargo", "test", "--test", "skill_eval_contract", "future_suffix"],
                    probe, run / f"logs/reject-{name}.log",
                )
                if code == 0 or "test result: FAILED" not in text:
                    raise SystemExit(f"{case}: broken {name} reader was not rejected by behavior checks")
        if source_path.read_text() != source:
            raise SystemExit("Archived input changed during smoke checks")
    run_evals.save_json(destination / "smoke-summary.json", {
        "wire_revision": run_evals.WIRE_REVISION,
        "evaluation_kind": "harness-smoke",
        "cases": results,
        "benchmarks_run": False,
        "negative_consumption_checks": "buf.len(), decoded size, and known size rejected in all three cases",
    })
    print("HARNESS SMOKE PASSED; not a model skill evaluation or performance measurement.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("historical_workspace", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    smoke(args.historical_workspace.resolve(), args.destination.resolve())


if __name__ == "__main__":
    main()
