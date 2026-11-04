"""Mutation ownership, immutable evidence, behavior gates, and timing policy."""

import copy
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import unittest
from unittest.mock import patch
import uuid

import run_evals


class ProjectTest(unittest.TestCase):
    def setUp(self):
        self.root = run_evals.REPO / "target" / "custom-vercodable-python-tests" / uuid.uuid4().hex
        self.root.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.root)

    def tag_candidate(self, case="create"):
        run_evals.save_json(self.root / "eval_metadata.json", {
            "wire_revision": run_evals.WIRE_REVISION, "eval_name": case,
        })


class WireFieldBenchmarkTests(unittest.TestCase):
    def test_libtest_discovery_lists_only_the_behavior_check(self):
        for flags, expected in [
            ([], "wire_fields_behavior: test\n"),
            (["--ignored"], ""),
            (["--measure"], "wire_fields_behavior: test\n"),
        ]:
            with self.subTest(flags=flags):
                result = subprocess.run(
                    ["cargo", "test", "-p", "vercode", "--bench", "wire_fields",
                     "--", "--list", "--format", "terse", *flags],
                    cwd=run_evals.REPO, capture_output=True, text=True, timeout=120,
                    env={**os.environ, "WIRE_FIELD_BENCH_ITERATIONS": "0"},
                    check=False,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, expected)


class PluginDistributionTests(ProjectTest):
    def test_marketplace_bundle_works_outside_repository(self):
        catalog = json.loads((run_evals.REPO / ".github/plugin/marketplace.json").read_text())
        entry = next(item for item in catalog["plugins"] if item["name"] == "vercode")
        source = (run_evals.REPO / entry["source"]).resolve()
        self.assertTrue(source.is_relative_to(run_evals.REPO / "vercode"))
        self.assertEqual(source / "skills/custom-vercodable", run_evals.SKILL)

        bundle = self.root / "standalone"
        shutil.copytree(source, bundle)
        manifest = json.loads((bundle / "plugin.json").read_text())
        self.assertEqual(manifest["name"], entry["name"])
        self.assertTrue((bundle / "LICENSE.txt").is_file())
        self.assertTrue((bundle / "skills/custom-vercodable/SKILL.md").is_file())
        for document in bundle.rglob("*.md"):
            for link in re.findall(r"\[[^\]]*\]\(([^)]+)\)", document.read_text()):
                if "://" in link or link.startswith("#"):
                    continue
                target = (document.parent / link.split("#", 1)[0]).resolve()
                with self.subTest(document=document.relative_to(bundle), link=link):
                    self.assertTrue(target.is_relative_to(bundle))
                    self.assertTrue(target.is_file())

    def test_cargo_package_includes_bundle_without_maintainer_assets(self):
        result = subprocess.run(
            ["cargo", "package", "-p", "vercode", "--list", "--allow-dirty"],
            cwd=run_evals.REPO, capture_output=True, text=True, timeout=120, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        packaged = set(result.stdout.splitlines())
        crate = run_evals.REPO / "vercode"
        bundle = crate / "agent-plugin"
        expected = {path.relative_to(crate).as_posix() for path in bundle.rglob("*") if path.is_file()}
        self.assertTrue(expected <= packaged, sorted(expected - packaged))
        evaluator_scripts = {path.name for path in (run_evals.EVALS / "scripts").glob("*.py")}
        for name in packaged:
            self.assertNotIn("skill-evals", Path(name).parts)
            if name.startswith("agent-plugin/"):
                self.assertTrue({"evals", "fixtures", "checks"}.isdisjoint(Path(name).parts), name)
                self.assertNotIn(Path(name).name, evaluator_scripts)


class MutationTests(unittest.TestCase):
    def test_each_mutation_changes_only_its_named_owner(self):
        for case, mutations in run_evals.MUTATIONS.items():
            source = (run_evals.EVALS / "fixtures" / f"{case}.rs").read_text()
            for mutation in mutations:
                with self.subTest(case=case, mutation=mutation[0]):
                    changed, count = run_evals.mutate(source, mutation)
                    self.assertEqual(count, 1)
                    self.assertNotEqual(source, changed)
                    start, end = run_evals.owner_span(source, mutation[1])
                    self.assertEqual(changed[:start], source[:start])
                    self.assertTrue(changed.endswith(source[end:]))

    def test_reorder_preserves_field_definitions(self):
        source = "pub struct Root {\n    pub first: u32,\n    pub second: u16,\n}\n"
        changed, count = run_evals.mutate(source, ("root", "Root", "reorder", "first", "second"))
        self.assertEqual(count, 1)
        self.assertIn("pub first: u32,", changed)
        self.assertIn("pub second: u16,", changed)
        self.assertLess(changed.index("pub second"), changed.index("pub first"))

    def test_entry_mutation_does_not_match_samples_point(self):
        source = (run_evals.EVALS / "fixtures" / "create.rs").read_text()
        mutation = next(item for item in run_evals.MUTATIONS["create"] if item[0] == "behind_vec")
        changed, count = run_evals.mutate(source, mutation)
        start, end = run_evals.owner_span(source, "Sample")
        self.assertEqual(count, 1)
        self.assertEqual(changed[start:end], source[start:end])
        start, end = run_evals.owner_span(changed, "Entry")
        self.assertLess(changed[start:end].index("weight"), changed[start:end].index("point"))

    def test_duplicate_container_fields_are_owner_scoped(self):
        source = "struct Frame { pub modes: Vec<Mode>, }\nstruct Extension { pub modes: Vec<Mode>, }"
        changed, count = run_evals.mutate(
            source, ("container", "Extension", "vec", "modes", ("Mode", "Strict")),
        )
        self.assertEqual(count, 1)
        self.assertEqual(changed, "struct Frame { pub modes: Vec<Mode>, }\n"
                         "struct Extension { pub modes: Vec<Strict>, }")

    def test_missing_target_is_not_a_successful_mutation(self):
        for source in ("struct Root;", "struct Other { pub first: u8, pub second: u8, }",
                       "struct Root { pub first: u8, }"):
            self.assertEqual(run_evals.mutate(source, ("root", "Root", "reorder", "first", "second")),
                             (source, 0))

    def test_owner_braces_in_comments_and_nested_variants(self):
        source = '// struct Mode { fake }\nenum Mode {\nUnknown,\nIdle,\nNamed { code: u32 }, // }\n}\n'
        changed, count = run_evals.mutate(source, ("variant", "Mode", "variant", "Unknown", "Idle"))
        self.assertEqual(count, 1)
        self.assertIn("Idle,\nUnknown,\nNamed { code: u32 }", changed)
        self.assertTrue(changed.startswith("// struct Mode { fake }"))

    def test_array_mutation_changes_only_owned_count(self):
        source = "struct Root { pub bytes: [u8; 8], pub other: [u8; 8], }"
        changed, count = run_evals.mutate(source, ("array", "Root", "array", "bytes", None))
        self.assertEqual(count, 1)
        self.assertEqual(changed, "struct Root { pub bytes: [u8; 9], pub other: [u8; 8], }")

    def test_type_errors_are_not_accepted_as_hash_tripwires(self):
        self.assertFalse(run_evals.guard_failed(1, "error[E0308]: custom-vercodable update required"))
        self.assertFalse(run_evals.guard_failed(1, "error[E0080]: overflow"))
        self.assertFalse(run_evals.guard_failed(0, "error[E0080]: custom-vercodable update required"))
        self.assertTrue(run_evals.guard_failed(1, "error[E0080]: custom-vercodable update required"))


def timing_workloads():
    return [{
        "elements": elements, "version": version, "samples": 9, "iterations": 20,
        "custom_bytes": 104 + elements * 8,
        "derive_bytes": 108 + elements * 16,
        "custom_ns": {method: [10.0] * 9 for method in ("write", "read_owned", "size")},
        "derive_ns": {method: [20.0] * 9 for method in ("write", "read_owned", "size")},
    } for elements in (32, 512, 4096) for version in (0, 2)]


class PerformancePolicyTests(unittest.TestCase):
    def test_all_methods_and_workloads_must_pass(self):
        workloads = timing_workloads()
        result = run_evals.summarize_performance(workloads)
        self.assertTrue(result["passed"])
        self.assertEqual(workloads[0]["comparison"]["write"]["paired_custom_wins"], 9)
        self.assertEqual(workloads[0]["savings_bytes"], 4 + 8 * 32)
        self.assertEqual(len(workloads[0]["custom_ns"]["write"]), 9)

    def test_one_slower_current_method_fails_separately_from_size(self):
        workloads = timing_workloads()
        workloads[-1]["custom_ns"]["read_owned"] = [30.0] * 9
        self.assertFalse(run_evals.summarize_performance(workloads)["passed"])
        self.assertTrue(workloads[-1]["size_smaller"])
        self.assertEqual(workloads[-1]["comparison"]["read_owned"]["verdict"], "slower_or_equal_median")

    def test_lower_median_without_seven_wins_is_inconclusive(self):
        workloads = timing_workloads()
        workloads[1]["custom_ns"]["size"] = [10.0] * 6 + [30.0] * 3
        self.assertFalse(run_evals.summarize_performance(workloads)["passed"])
        self.assertEqual(workloads[1]["comparison"]["size"]["verdict"], "inconclusive_paired_wins")

    def test_seven_paired_wins_are_sufficient(self):
        workloads = timing_workloads()
        workloads[1]["custom_ns"]["size"] = [10.0] * 7 + [30.0] * 2
        self.assertTrue(run_evals.summarize_performance(workloads)["passed"])

    def test_equal_or_larger_size_fails_even_with_fast_timings(self):
        workloads = timing_workloads()
        workloads[1]["custom_bytes"] = workloads[1]["derive_bytes"]
        self.assertFalse(run_evals.summarize_performance(workloads)["passed"])
        self.assertTrue(workloads[1]["comparison"]["write"]["custom_faster_accepted"])

    def test_base_results_do_not_control_current_acceptance(self):
        workloads = timing_workloads()
        workloads[0]["custom_ns"]["size"] = [40.0] * 9
        self.assertTrue(run_evals.summarize_performance(workloads)["passed"])
        self.assertFalse(workloads[0]["performance_accepted"])

    def test_missing_duplicate_or_malformed_samples_are_rejected(self):
        for workloads in (timing_workloads()[:-1], [copy.deepcopy(timing_workloads()[0])] * 6):
            with self.assertRaises(ValueError):
                run_evals.summarize_performance(workloads)
        for invalid in ([1.0] * 8, [float("nan")] * 9, [0.0] * 9):
            workloads = timing_workloads()
            workloads[0]["custom_ns"]["write"] = invalid
            with self.assertRaises(ValueError):
                run_evals.summarize_performance(workloads)


class RunnerTests(ProjectTest):
    def test_legacy_workspace_is_read_only(self):
        for workspace in ("LEGACY_WORKSPACE", "EXTERNAL_WORKSPACE"):
            with patch.object(run_evals, workspace, self.root):
                with self.assertRaisesRegex(SystemExit, "Historical"):
                    run_evals.prepare("create", self.root / "new-output")
                with self.assertRaises(SystemExit):
                    run_evals.save_json(self.root / "grading.json", {})
        self.assertEqual(list(self.root.iterdir()), [])

    def test_copied_historical_run_cannot_be_regraded_or_relabelled(self):
        output = self.root / "outputs"
        output.mkdir()
        metadata = self.root / "eval_metadata.json"
        metadata.write_text(json.dumps({"wire_revision": "global-root-passes-v2", "eval_name": "create"}))
        before = metadata.read_bytes()
        with self.assertRaisesRegex(SystemExit, "Historical"):
            run_evals.inject_contract("create", output)
        with self.assertRaisesRegex(SystemExit, "Historical"):
            run_evals.save_json(metadata, {"wire_revision": run_evals.WIRE_REVISION})
        self.assertEqual(metadata.read_bytes(), before)
        self.assertEqual(list(output.iterdir()), [])

    def test_archived_reports_are_read_only(self):
        with patch.object(run_evals, "EVALS", self.root):
            for name in ("historical-external-results.json", "legacy-framed-results.json"):
                path = self.root / name
                path.write_text('{"historical": true}\n')
                with self.assertRaisesRegex(SystemExit, "Historical"):
                    run_evals.save_json(path, {"wire_revision": run_evals.WIRE_REVISION})
                self.assertEqual(path.read_text(), '{"historical": true}\n')

    def test_injection_requires_current_revision_and_matching_case(self):
        output = self.root / "outputs"
        output.mkdir()
        with self.assertRaisesRegex(SystemExit, "Missing"):
            run_evals.inject_contract("create", output)
        self.tag_candidate("update")
        with self.assertRaisesRegex(SystemExit, "Expected create"):
            run_evals.inject_contract("create", output)
        (self.root / "eval_metadata.json").write_text(json.dumps({"eval_name": "create"}))
        with self.assertRaisesRegex(SystemExit, "wire_revision"):
            run_evals.inject_contract("create", output)

    def test_prepare_freezes_update_and_records_revision(self):
        output = self.root / "outputs"
        with patch("builtins.print"):
            run_evals.prepare("update", output)
        frozen = (run_evals.EVALS / "fixtures" / "update.rs").read_bytes()
        self.assertEqual((output / "historical.rs").read_bytes(), frozen)
        self.assertEqual((output / "src" / "lib.rs").read_bytes(), frozen)
        metadata = json.loads((self.root / "eval_metadata.json").read_text())
        self.assertEqual(metadata["wire_revision"], run_evals.WIRE_REVISION)
        self.assertIn('bincode = "=1.3.3"', (output / "Cargo.toml").read_text())
        self.assertIn(f"path = {json.dumps(str(run_evals.REPO / 'vercode'))}",
                      (output / "Cargo.toml").read_text())

    def test_injected_checks_preserve_candidate_authored_tests_and_source(self):
        output = self.root / "outputs"
        with patch("builtins.print"):
            run_evals.prepare("create", output)
        (output / "tests").mkdir()
        (output / "tests" / "contract.rs").write_text("// candidate-owned\n")
        before = (output / "src" / "lib.rs").read_bytes()
        names = run_evals.inject_contract("create", output)
        self.assertGreaterEqual(len(names), 12)
        self.assertEqual((output / "tests" / "contract.rs").read_text(), "// candidate-owned\n")
        self.assertEqual((output / "src" / "lib.rs").read_bytes(), before)
        self.assertTrue((output / "tests" / "skill_eval_contract.rs").exists())

    def test_grade_probes_actual_children_and_never_repairs_candidate_library(self):
        output = self.root / "outputs"
        with patch("builtins.print"):
            run_evals.prepare("create", output)
        before = (output / "src" / "lib.rs").read_bytes()
        names = run_evals.inject_contract("create", output)
        children = []

        def fake_command(args, cwd, log):
            if "--example" in args:
                probe = (cwd / "examples" / "eval_no_nested_codec.rs").read_text()
                children.append(probe)
                return 1, f"error[E0277]: VerCodable not implemented\n{probe}", 0.1
            if "--test" in args:
                return 0, "\n".join(f"test {name} ... ok" for name in names), 0.1
            if cwd != output and "--lib" in args:
                return 1, "error[E0080]: type tree changed; custom-vercodable update required", 0.1
            return 0, "", 0.1

        with patch.object(run_evals, "command", side_effect=fake_command):
            with patch("builtins.print"):
                self.assertTrue(run_evals.grade("create", output, self.root))
        self.assertEqual(len(children), 2)
        self.assertIn("requires_codec::<custom_vercodable_eval::Point>()", children[0])
        self.assertIn("requires_codec::<custom_vercodable_eval::Entry>()", children[1])
        self.assertEqual((output / "src" / "lib.rs").read_bytes(), before)
        grade = json.loads((self.root / "grading.json").read_text())
        self.assertEqual(grade["summary"]["total"], len(names) + 5 + len(run_evals.MUTATIONS["create"]))

    def test_benchmark_requires_explicit_idle_confirmation(self):
        with self.assertRaisesRegex(SystemExit, "confirm-no-evaluation-jobs"):
            run_evals.benchmark(self.root, self.root)

    def test_failed_behavior_blocks_timing_and_records_failure(self):
        self.tag_candidate()
        output = self.root / "outputs"
        output.mkdir()
        with patch.object(run_evals, "command", return_value=(1, "compile failed", 0.1)) as command:
            with patch("builtins.print"):
                self.assertFalse(run_evals.benchmark(output, self.root, confirm_idle=True))
        self.assertEqual(command.call_count, 1)
        report = json.loads((self.root / "performance.json").read_text())
        self.assertFalse(report["behavior_gate_passed"])
        self.assertFalse(report["performance_acceptance"]["passed"])
        self.assertNotIn("workloads", report)

    def test_guard_scores_do_not_block_successful_behavior_benchmark(self):
        self.tag_candidate()
        output = self.root / "outputs"
        (output / "src").mkdir(parents=True)
        (output / "src" / "lib.rs").write_text("// candidate source, never repaired")
        run_evals.save_json(self.root / "grading.json", {"summary": {"failed": 2}})
        names = run_evals.inject_contract("create", output)
        ok = "\n".join(f"test {name} ... ok" for name in names)
        measured = "\n".join(json.dumps(item, separators=(",", ":")) for item in timing_workloads())
        with patch.object(run_evals, "command", side_effect=[(0, ok, 1), (0, measured, 1), (0, "rustc", 1)]):
            with patch("builtins.print"):
                self.assertTrue(run_evals.benchmark(output, self.root, confirm_idle=True))
        report = json.loads((self.root / "performance.json").read_text())
        self.assertTrue(report["behavior_gate_passed"])
        self.assertTrue(report["performance_acceptance"]["passed"])
        self.assertEqual((output / "src" / "lib.rs").read_text(), "// candidate source, never repaired")
        self.assertEqual(json.loads((self.root / "grading.json").read_text()), {"summary": {"failed": 2}})

    def test_reporting_omits_unavailable_resource_metrics(self):
        report = {
            "metadata": {},
            "runs": [{
                "eval_id": 1, "configuration": "with_skill", "run_number": 1,
                "result": {"passed": 1, "total": 2, "time_seconds": 0, "tokens": 0},
            }],
            "run_summary": {"with_skill": {"tokens": {"mean": 0}, "pass_rate": {"mean": 0.5}}},
        }
        snapshot = self.root / "snapshot"
        snapshot.mkdir()
        (snapshot / "SKILL.md").write_text("evaluated snapshot")
        (self.root / "benchmark.json").write_text(json.dumps(report))
        (self.root / "analysis.json").write_text(json.dumps(["Observed limitation."]))
        run_evals.review_metadata(self.root, "recorded-model", snapshot)
        run_evals.review_metadata(self.root, "recorded-model", snapshot)
        actual = json.loads((self.root / "benchmark.json").read_text())
        self.assertNotIn("tokens", actual["runs"][0]["result"])
        self.assertNotIn("time_seconds", actual["runs"][0]["result"])
        self.assertNotIn("tokens", actual["run_summary"]["with_skill"])
        self.assertEqual(actual["runs"][0]["result"]["passed"], 1)
        self.assertEqual(actual["metadata"]["runs_per_configuration"], 1)
        self.assertEqual(actual["metadata"]["skill_files_sha256"], {
            "SKILL.md": hashlib.sha256(b"evaluated snapshot").hexdigest(),
        })
        self.assertEqual(actual["notes"].count("Observed limitation."), 1)

    def test_records_preserve_grades_performance_and_answer_identity(self):
        grade = {
            "wire_revision": run_evals.WIRE_REVISION,
            "summary": {"passed": 0, "total": 1},
            "expectations": [{"text": "guard", "passed": False, "evidence": "failure log"}],
        }
        workspace = self.root / "workspace"
        iteration = workspace / "iteration-1"
        run = iteration / "eval-decisions" / "without_skill" / "run-1"
        (run / "outputs").mkdir(parents=True)
        run_evals.save_json(iteration / "benchmark.json", {"metadata": {
            "executor_model": "model", "wire_revision": run_evals.WIRE_REVISION,
        }})
        run_evals.save_json(run / "grading.json", grade)
        run_evals.save_json(run / "performance.json", {
            "wire_revision": run_evals.WIRE_REVISION, "performance_acceptance": {"passed": False},
        })
        previous = {"wire_revision": run_evals.WIRE_REVISION, "reason": "Insufficient timing duration"}
        run_evals.save_json(run / "performance-initial.json", previous)
        (run / "outputs" / "answers.json").write_text("[]")
        destination = self.root / "results.json"
        with patch.object(run_evals, "REPO", self.root):
            run_evals.record_results(workspace, destination)
        actual = json.loads(destination.read_text())["iterations"][0]
        self.assertEqual(actual["metadata"]["executor_model"], "model")
        recorded = actual["runs"][0]
        self.assertEqual(recorded["summary"], grade["summary"])
        self.assertEqual(recorded["expectations"], grade["expectations"])
        self.assertFalse(recorded["performance"]["performance_acceptance"]["passed"])
        self.assertEqual(recorded["superseded_performance"]["performance-initial.json"],
                         previous)
        self.assertEqual(recorded["source_sha256"]["outputs/answers.json"], hashlib.sha256(b"[]").hexdigest())
        self.assertEqual(recorded["artifact_path"], str(run.relative_to(self.root)))

    def test_catalog_has_three_executable_twelve_decision_and_twenty_routing_cases(self):
        catalog = json.loads((run_evals.EVALS / "evals.json").read_text())
        self.assertEqual(catalog["wire_revision"], run_evals.WIRE_REVISION)
        self.assertEqual([entry["name"] for entry in catalog["evals"][:3]], list(run_evals.CASES))
        self.assertEqual(len(catalog["evals"][3:]), 12)
        self.assertEqual(len(json.loads((run_evals.EVALS / "trigger-evals.json").read_text())), 20)
        for entry in catalog["evals"]:
            for name in entry["files"]:
                self.assertTrue((run_evals.EVALS / name).is_file(), name)
        for case in run_evals.CASES:
            tests = re.findall(r"#\[test\]\s*fn (\w+)", (run_evals.EVALS / "checks" / f"{case}.rs").read_text())
            self.assertGreaterEqual(len(tests), 12)

    def test_results_cannot_rebrand_historical_scores(self):
        workspace = self.root / "workspace"
        iteration = workspace / "iteration-1"
        iteration.mkdir(parents=True)
        (iteration / "benchmark.json").write_text(json.dumps({
            "metadata": {"wire_revision": "global-root-passes-v2"},
        }))
        destination = self.root / "results.json"
        with self.assertRaisesRegex(SystemExit, "wire_revision"):
            run_evals.record_results(workspace, destination)
        self.assertFalse(destination.exists())

    def test_results_reject_mixed_grades_and_harness_smoke(self):
        workspace = self.root / "workspace"
        iteration = workspace / "iteration-1"
        run = iteration / "eval-create/with_skill/run-1"
        run.mkdir(parents=True)
        (iteration / "benchmark.json").write_text(json.dumps({
            "metadata": {"wire_revision": run_evals.WIRE_REVISION},
        }))
        destination = self.root / "results.json"
        for grade, diagnostic in [
            ({"wire_revision": "global-root-passes-v2"}, "wire_revision"),
            ({"wire_revision": run_evals.WIRE_REVISION, "evaluation_kind": "harness-smoke"}, "smoke"),
        ]:
            with self.subTest(grade=grade):
                (run / "grading.json").write_text(json.dumps(grade))
                with self.assertRaisesRegex(SystemExit, diagnostic):
                    run_evals.record_results(workspace, destination)
                self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
