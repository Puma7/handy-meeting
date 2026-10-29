"""Synthetic event fixtures; no GitHub mutations, builds or hardware claims."""
import ast
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
    """Read and validate every PR filter before a caller can short-circuit on a match."""
    section = re.search(r'^  pull_request:\n(.*?)(?=^  [\w-]+:|\Z)', text, re.M | re.S).group(1)
    inline = re.search(r'^    paths: (\[.*\])$', section, re.M)
    if inline:
        patterns = ast.literal_eval(inline.group(1))
    else:
        # The whole block up to the next key at the same or a lower level: blank and comment
        # lines are skipped, entries are read, and anything else fails closed.
        # The block ends at the next key; a wrongly indented list entry stays in it and is rejected.
        block = re.search(r'^    paths:\n(.*?)(?=^ {0,3}\S|^    [A-Za-z_][\w-]*:|\Z)', section, re.M | re.S)
        if not block:
            raise ValueError('No readable pull_request paths filter')
        patterns = []
        for line in block.group(1).splitlines():
            if not line.strip() or line.strip().startswith('#'):
                continue
            if not line.startswith('      - '):
                raise ValueError('Unreadable pull_request paths line: ' + line)
            patterns.append(block_entry(line[len('      - '):]))
        if not patterns:
            raise ValueError('Empty pull_request paths filter')
    for pattern in patterns:
        path_matches('', pattern)
    return patterns


def block_entry(entry):
    """Read one block-list entry in either quote style; anything else fails closed."""
    match = re.fullmatch(r"'([^']+)'|\"([^\"\\]+)\"", entry.strip())
    if not match:
        raise ValueError('Unreadable pull_request path entry: ' + entry)
    return match.group(1) if match.group(1) is not None else match.group(2)


def path_matches(path, pattern):
    """Match the literal/*/** subset used here, keeping single stars inside one path segment."""
    if any(char in pattern for char in '?+[]!{}'):
        raise ValueError('Add explicit coverage before using another GitHub path-filter feature')
    expression = re.escape(pattern).replace(r'\*\*/', '(?:.*/)?').replace(r'\*\*', '.*').replace(r'\*', '[^/]*')
    return re.fullmatch(expression, path, flags=re.DOTALL) is not None


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
    def test_single_star_does_not_mask_missing_recursive_platform_coverage(self):
        nested = 'platform/macos/AudioCapture.swift'
        self.assertFalse(path_matches(nested, 'platform/*'))
        self.assertTrue(path_matches(nested, 'platform/**'))
        self.assertTrue(path_matches('platform/helper.swift', 'platform/*'))

    def test_globstar_directory_matches_zero_or_multiple_directories(self):
        for path in ('apps/config.json', 'apps/desktop/config.json', 'apps/desktop/nested/config.json'):
            self.assertTrue(path_matches(path, 'apps/**/config.json'))
        self.assertFalse(path_matches('apps/desktop/config.json', 'apps/*.json'))

    def test_unimplemented_pattern_features_fail_closed(self):
        for pattern in ('!platform/**', 'platform/?.swift', 'platform/[ab].swift', 'platform/{a,b}.swift'):
            with self.assertRaises(ValueError):
                path_matches('platform/a.swift', pattern)

    def test_unsupported_filter_after_positive_match_rejects_real_workflows(self):
        for name in ('rust.yml', 'rust-portable.yml'):
            text = workflow(name)
            for unsupported in ('!platform/macos/**', 'platform/?.swift', 'platform/[ab].swift'):
                separator = ', ' if '    paths: [' in text else '\n      - '
                mutated = text.replace("'platform/**'", "'platform/**'" + separator + repr(unsupported))
                self.assertNotEqual(mutated, text, name)
                with self.subTest(workflow=name, pattern=unsupported), self.assertRaises(ValueError):
                    patterns = pr_paths(mutated)
                    any(path_matches('platform/macos/AudioCapture.swift', pattern) for pattern in patterns)

    def test_block_list_entries_in_both_quote_styles_are_read_and_validated(self):
        head, pr_section = workflow('rust.yml').split('\n  pull_request:', 1)
        self.assertNotIn('    paths: [', pr_section.split('permissions:', 1)[0])

        def with_entry(entry):
            """Insert one entry into the pull_request block list only, after 'platform/**'."""
            mutated = pr_section.replace("      - 'platform/**'\n", "      - 'platform/**'\n" + entry, 1)
            self.assertNotEqual(mutated, pr_section)
            return head + '\n  pull_request:' + mutated

        text = workflow('rust.yml')
        quoted = with_entry('      - "docs/**"\n')
        self.assertEqual(len(pr_paths(quoted)), len(pr_paths(text)) + 1)
        self.assertIn('docs/**', pr_paths(quoted))
        # A double-quoted exclusion must not be skipped silently.
        with self.assertRaises(ValueError):
            pr_paths(with_entry('      - "!platform/macos/**"\n'))
        # A blank or comment line must not hide a later entry from validation.
        with self.assertRaises(ValueError):
            pr_paths(with_entry("\n      # later entries\n      - '!platform/macos/**'\n"))
        # A four-space entry must not end the block early and escape validation.
        with self.assertRaises(ValueError):
            pr_paths(with_entry("    - '!platform/macos/**'\n"))
        separated = with_entry("\n      - 'docs/**'\n")
        self.assertIn('docs/**', pr_paths(separated))
        for unreadable in ('platform/macos/**', "'platform/**' # note", '"platform/**'):
            with self.subTest(entry=unreadable), self.assertRaises(ValueError):
                pr_paths(with_entry('      - ' + unreadable + '\n'))

    def test_legacy_source_only_ready_changes_trigger_both_candidate_workflows(self):
        for changed in ('src/HandyMeeting/Models.cs', 'verify.ps1'):
            for name in ('rust.yml', 'rust-portable.yml'):
                self.assertTrue(any(path_matches(changed, pattern) for pattern in pr_paths(workflow(name))), (changed, name))

    def test_platform_only_ready_changes_trigger_both_candidate_workflows(self):
        for name in ('rust.yml', 'rust-portable.yml'):
            self.assertTrue(any(path_matches('platform/macos/AudioCapture.swift', pattern) for pattern in pr_paths(workflow(name))), name)

    def test_both_candidate_workflow_definitions_and_native_inputs_share_pr_filters(self):
        windows, portable = (pr_paths(workflow(name)) for name in ('rust.yml', 'rust-portable.yml'))
        self.assertEqual(set(windows), set(portable))
        for changed in ('.github/workflows/rust.yml', '.github/workflows/rust-portable.yml', 'global.json'):
            for patterns in (windows, portable):
                self.assertTrue(any(path_matches(changed, pattern) for pattern in patterns), changed)

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
        self.assertIn('if [ "$COMPLETE" = true ]; then exit 0; fi', validate)
        self.assertIn('exit 1', validate)
        # Releases published on GitHub with a new tag: the tag run of the same commit builds.
        refusal = validate[validate.index('Refuse compilation on publication'):]
        self.assertIn('actions/workflows/rust-release.yml/runs?event=push&head_sha=$GITHUB_SHA', refusal)
        self.assertIn('actions: read', validate)
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

    def test_every_release_job_takes_the_version_from_the_tag(self):
        # 1.0.7: the tag was created without bumping the files, and the build refused it.
        text = workflow('rust-release.yml')
        self.assertIn('scripts/set-version.py "$REQUESTED_TAG"', job(text, 'validate'))
        self.assertNotIn('Tag must match all application versions', text)
        for name in ('windows', 'portable'):
            self.assertIn('version: ${{ needs.validate.outputs.version }}', job(text, name))
        for name in ('publish', 'publish_existing'):
            body = job(text, name)
            self.assertLess(body.index('scripts/set-version.py'), body.index('source-fingerprint'), name)
        for name, job_name in (('rust.yml', 'windows'), ('rust-portable.yml', 'desktop')):
            body = job(workflow(name), job_name)
            self.assertIn('if: inputs.version', body)
            self.assertLess(body.index('set-version.py'), body.index('Install pinned Rust tooling'), name)

    def test_release_artifact_retention_and_current_run_binding_remain(self):
        text = workflow('rust-release.yml')
        self.assertIn('cancel-in-progress: false', text)
        self.assertEqual(text.count('gh run download "$GITHUB_RUN_ID"'), 8)
        self.assertIn('retention-days: 30', text)
        self.assertIn('node scripts/rust-artifacts.mjs source-fingerprint', text)
        self.assertIn("commit == os.environ['GITHUB_SHA']", text)


if __name__ == '__main__':
    unittest.main()
