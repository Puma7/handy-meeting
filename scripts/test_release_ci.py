"""Synthetic event fixtures; no GitHub mutations, builds or hardware claims."""
import ast
import fnmatch
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / 'scripts/ci-plan.py'
spec = importlib.util.spec_from_file_location('ci_plan', SCRIPT)
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


def workflow(name):
    return (ROOT / '.github/workflows' / name).read_text(encoding='utf-8')


def job(text, name):
    """Read a specific job so a guard elsewhere cannot satisfy its boundary assertions."""
    match = re.search(r'^  ' + re.escape(name) + r':\n(.*?)(?=^  [\w-]+:|\Z)', text, re.M | re.S)
    if not match:
        raise AssertionError('Missing workflow job: ' + name)
    return match.group(1)


def pr_paths(text):
    """Read the repository's inline/block PR filters for real changed-path fixtures."""
    section = re.search(r'^  pull_request:\n(.*?)(?=^  [\w-]+:|\Z)', text, re.M | re.S).group(1)
    inline = re.search(r'^    paths: (\[.*\])$', section, re.M)
    if inline:
        return ast.literal_eval(inline.group(1))
    return re.findall(r"^      - '([^']+)'$", section, re.M)


class EventRouting(unittest.TestCase):
    def test_draft_updates_never_build_packages_even_with_a_truthy_input(self):
        for action in ('opened', 'synchronize', 'reopened', 'converted_to_draft'):
            for requested in ('', 'false', 'true'):
                self.assertFalse(policy.plan('pull_request', {'action': action, 'pull_request': {'draft': True}}, requested)['packages'])

    def test_promotion_and_later_ready_updates_require_complete_candidates(self):
        for action in ('ready_for_review', 'synchronize', 'reopened', 'opened'):
            self.assertTrue(policy.plan('pull_request', {'action': action, 'pull_request': {'draft': False}}, '')['packages'])

    def test_main_is_source_only_and_release_reusable_calls_opt_in(self):
        self.assertFalse(policy.plan('push', {'ref': 'refs/heads/main'}, '')['packages'])
        self.assertTrue(policy.plan('push', {'ref': 'refs/tags/v1.0.5'}, 'true')['packages'])

    def test_manual_source_only_is_distinct_from_explicit_full_run(self):
        self.assertFalse(policy.plan('workflow_dispatch', {}, 'false')['packages'])
        self.assertTrue(policy.plan('workflow_dispatch', {}, 'true')['packages'])

    def test_missing_or_ambiguous_draft_state_fails_closed(self):
        for state in ({}, {'draft': 'false'}, {'draft': 0}, {'draft': None}):
            with self.assertRaises((KeyError, ValueError)):
                policy.plan('pull_request', {'pull_request': state}, '')

    def test_unknown_input_and_privileged_pr_event_are_refused(self):
        with self.assertRaises(ValueError):
            policy.plan('push', {}, 'yes')
        with self.assertRaises(ValueError):
            policy.plan('pull_request_target', {'pull_request': {'draft': False}}, 'true')

    def test_real_cli_writes_scope_without_claiming_skipped_checks(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            event = root / 'event.json'
            event.write_text(json.dumps({'pull_request': {'draft': True}}), encoding='utf-8')
            output, summary = root / 'output', root / 'summary'
            env = dict(os.environ, GITHUB_EVENT_NAME='pull_request', GITHUB_EVENT_PATH=str(event),
                       GITHUB_OUTPUT=str(output), GITHUB_STEP_SUMMARY=str(summary), CI_PACKAGES='')
            result = subprocess.run([sys.executable, '-I', '-S', str(SCRIPT)], env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(output.read_text(), 'packages=false\n')
            self.assertIn('not executed', summary.read_text())
            self.assertIn('No downloadable candidate', summary.read_text())


class WorkflowBoundaries(unittest.TestCase):
    def test_legacy_source_only_ready_changes_trigger_both_candidate_workflows(self):
        for changed in ('src/HandyMeeting/Models.cs', 'verify.ps1'):
            for name in ('rust.yml', 'rust-portable.yml'):
                self.assertTrue(any(fnmatch.fnmatchcase(changed, pattern) for pattern in pr_paths(workflow(name))), (changed, name))

    def test_platform_only_ready_changes_trigger_both_candidate_workflows(self):
        for name in ('rust.yml', 'rust-portable.yml'):
            self.assertTrue(any(fnmatch.fnmatchcase('platform/macos/AudioCapture.swift', pattern) for pattern in pr_paths(workflow(name))), name)

    def test_both_candidate_workflow_definitions_and_native_inputs_share_pr_filters(self):
        windows, portable = (pr_paths(workflow(name)) for name in ('rust.yml', 'rust-portable.yml'))
        self.assertEqual(set(windows), set(portable))
        for changed in ('.github/workflows/rust.yml', '.github/workflows/rust-portable.yml', 'global.json'):
            for patterns in (windows, portable):
                self.assertTrue(any(fnmatch.fnmatchcase(changed, pattern) for pattern in patterns), changed)

    def test_ready_transition_is_subscribed_and_shared_plan_controls_both_workflows(self):
        for name in ('rust.yml', 'rust-portable.yml'):
            text = workflow(name)
            self.assertIn('ready_for_review, converted_to_draft', text)
            self.assertIn('CI_PACKAGES: ${{ inputs.packages }}', job(text, 'plan'))
            self.assertIn('run: python3 scripts/ci-plan.py', job(text, 'plan'))
            self.assertIn('packages: ${{ steps.plan.outputs.packages }}', job(text, 'plan'))

    def test_every_windows_release_step_including_upload_depends_on_full_mode(self):
        windows = job(workflow('rust.yml'), 'windows')
        self.assertIn('needs: plan', windows)
        steps = re.split(r'^      - name: ', windows, flags=re.M)[1:]
        first = next(index for index, step in enumerate(steps) if step.startswith('Build release from'))
        for step in steps[first:]:
            if step.startswith('Keep model-free verification records'):
                self.assertIn('if: always()', step)
            else:
                self.assertIn("if: needs.plan.outputs.packages == 'true'", step, step.splitlines()[0])
        for required in ('./verify.ps1', 'cargo test --workspace --all-targets --locked', 'cargo clippy --workspace --all-targets --locked -- -D warnings'):
            self.assertIn(required, windows)
        self.assertNotIn("if: needs.plan.outputs.packages", windows[:windows.index('Build release from')])

    def test_full_portable_matrix_is_retained_but_not_on_draft_or_main(self):
        text = workflow('rust-portable.yml')
        self.assertNotIn('\n  push:', text[:text.index('permissions:')])
        desktop = job(text, 'desktop')
        self.assertIn("if: needs.plan.outputs.packages == 'true'", desktop)
        for target in ('linux-x64', 'osx-arm64', 'osx-x64'):
            self.assertIn('rid: ' + target, desktop)
        for command in ('test', 'clippy'):
            self.assertIn('cargo +1.91.1 ' + command + ' -p handy-bridge -p meeting-core -p meeting-notes --all-targets --no-default-features --locked', desktop)
        self.assertIn("if: needs.plan.outputs.packages != 'true'", job(workflow('rust.yml'), 'portable-core'))

    def test_published_event_cannot_fall_back_to_building_or_uploading_new_bytes(self):
        text = workflow('rust-release.yml')
        for name in ('windows', 'portable', 'publish'):
            self.assertIn("github.event_name != 'release'", job(text, name))
        for name in ('windows', 'portable'):
            self.assertIn('packages: true', job(text, name))
        validate = job(text, 'validate')
        self.assertIn("if: github.event_name == 'release'", validate)
        self.assertIn('if [ "$COMPLETE" != true ]; then', validate)
        self.assertIn('exit 1', validate)
        existing = job(text, 'publish_existing')
        self.assertIn("already_complete == 'true'", existing)
        self.assertIn('Complete exact-source release required immediately before promotion', existing)
        self.assertNotIn('--publish', existing)

    def test_required_verify_context_remains_without_the_push_plus_pr_duplicate(self):
        text = workflow('windows.yml')
        triggers = text.split('permissions:', 1)[0]
        self.assertIn('workflow_dispatch:', triggers)
        self.assertNotIn('\n  push:', triggers)
        self.assertIn('\n  pull_request:', triggers)
        self.assertIn('  verify:', text)
        self.assertIn('run: ./verify.ps1', text)
        self.assertIn('./verify.ps1', job(workflow('rust.yml'), 'windows'))

    def test_release_artifact_retention_and_current_run_binding_remain(self):
        text = workflow('rust-release.yml')
        self.assertIn('cancel-in-progress: false', text)
        self.assertEqual(text.count('gh run download "$GITHUB_RUN_ID"'), 8)
        self.assertIn('retention-days: 30', text)
        self.assertIn('node scripts/rust-artifacts.mjs source-fingerprint', text)
        self.assertIn("commit == os.environ['GITHUB_SHA']", text)


if __name__ == '__main__':
    unittest.main()
