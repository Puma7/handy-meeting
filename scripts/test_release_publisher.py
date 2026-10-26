"""Synthetic GitHub responses only; never publish or contact a real service."""
import json
import pathlib
import unittest
from unittest import mock

import test_release_packages as fixtures


class DraftReleaseRecovery(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.ReleaseGuards()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.fixture.assemble()
        self.publisher = fixtures.publisher
        self.release = {'id': 41, 'tag_name': 'v1.0.0', 'draft': True,
                        'assets': [], 'html_url': 'https://example.test/draft/41'}
        manifest = fixtures.packages.read(self.fixture.output / 'RELEASE_MANIFEST.json')
        self.rows = manifest['files'] + [
            {'name': name, 'bytes': (self.fixture.output / name).stat().st_size,
             'sha256': fixtures.packages.sha(self.fixture.output / name)}
            for name in ['RELEASE_MANIFEST.json', 'SHA256SUMS.txt']]
        self.calls = []

    def cli(self, *args, **kwargs):
        self.calls.append((args, kwargs))
        if args == ('api', 'repos/owner/repo/commits/v1.0.0'):
            return json.dumps({'sha': fixtures.COMMIT})
        if args == ('api', 'repos/owner/repo/releases/tags/v1.0.0'):
            return None  # GitHub's tag endpoint exposes published releases only.
        if args == ('api', 'repos/owner/repo/releases?per_page=100&page=1'):
            return json.dumps([self.release])
        if args == ('api', 'repos/owner/repo/releases/41'):
            return json.dumps(self.release)
        if args[:3] == ('api', '--method', 'POST'):
            self.assertIn('/releases/41/assets?name=', args[3])
            source = pathlib.Path(args[args.index('--input') + 1])
            row = next(row for row in self.rows if row['name'] == source.name)
            self.assertEqual(fixtures.packages.sha(source), row['sha256'])
            self.release['assets'].append({'id': 1000 + len(self.release['assets']),
                'name': row['name'], 'size': row['bytes'], 'digest': 'sha256:' + row['sha256']})
            return '{}'
        raise AssertionError('Unexpected GitHub operation: ' + repr(args[:3]))

    def test_existing_draft_is_reused_without_creating_or_publishing_another(self):
        with mock.patch.object(self.publisher, 'gh', side_effect=self.cli):
            self.publisher.publish(self.fixture.output, 'owner/repo', 'v1.0.0',
                                   fixtures.COMMIT, self.fixture.root / 'notes.md', False)
        self.assertEqual(len(self.release['assets']), 17)
        self.assertTrue(self.release['draft'])
        self.assertFalse(any(args[:2] == ('release', 'create') for args, _ in self.calls))
        self.assertFalse(any('PATCH' in args for args, _ in self.calls))

    def test_duplicate_drafts_fail_before_any_upload_or_mutation(self):
        def cli(*args, **kwargs):
            if args == ('api', 'repos/owner/repo/releases?per_page=100&page=1'):
                return json.dumps([self.release, dict(self.release, id=42)])
            return self.cli(*args, **kwargs)
        with mock.patch.object(self.publisher, 'gh', side_effect=cli):
            with self.assertRaisesRegex(ValueError, 'Multiple releases'):
                self.publisher.publish(self.fixture.output, 'owner/repo', 'v1.0.0',
                                       fixtures.COMMIT, self.fixture.root / 'notes.md', False)
        self.assertEqual(self.release['assets'], [])
        self.assertFalse(any('POST' in args or 'PATCH' in args for args, _ in self.calls))

    def test_explicit_release_id_never_uses_ambiguous_tag_lookup(self):
        with mock.patch.object(self.publisher, 'gh', side_effect=self.cli):
            self.publisher.publish(self.fixture.output, 'owner/repo', 'v1.0.0',
                                   fixtures.COMMIT, self.fixture.root / 'notes.md', False,
                                   release_id=41)
        self.assertEqual(len(self.release['assets']), 17)
        self.assertFalse(any('/releases/tags/' in str(args) or '?per_page=' in str(args)
                             for args, _ in self.calls))

    def test_explicit_id_for_another_tag_is_rejected_without_upload(self):
        self.release['tag_name'] = 'v2.0.0'
        with mock.patch.object(self.publisher, 'gh', side_effect=self.cli):
            with self.assertRaisesRegex(ValueError, 'Release identity'):
                self.publisher.publish(self.fixture.output, 'owner/repo', 'v1.0.0',
                                       fixtures.COMMIT, self.fixture.root / 'notes.md', False,
                                       release_id=41)
        self.assertEqual(self.release['assets'], [])

    def test_draft_on_later_page_is_found_and_every_page_checked(self):
        filler = [{'id': i + 100, 'tag_name': 'v0.0.' + str(i)} for i in range(100)]
        def cli(*args, **kwargs):
            if args == ('api', 'repos/owner/repo/releases?per_page=100&page=1'):
                self.calls.append((args, kwargs))
                return json.dumps(filler)
            if args == ('api', 'repos/owner/repo/releases?per_page=100&page=2'):
                self.calls.append((args, kwargs))
                return json.dumps([self.release])
            return self.cli(*args, **kwargs)
        with mock.patch.object(self.publisher, 'gh', side_effect=cli):
            found = self.publisher.find_release('owner/repo', 'v1.0.0')
        self.assertEqual(found['id'], 41)
        self.assertTrue(any('page=2' in str(args) for args, _ in self.calls))

    def test_binary_download_is_bound_to_asset_id_and_new_output(self):
        target = self.fixture.root / 'manifest.json'
        with mock.patch.object(self.publisher, 'gh', return_value='') as cli:
            self.publisher.download_asset('owner/repo', {'id': 92}, target)
        cli.assert_called_once_with('api', 'repos/owner/repo/releases/assets/92',
            '-H', 'Accept: application/octet-stream', output_path=target)

    def test_existing_binary_output_is_preserved_before_starting_a_request(self):
        target = self.fixture.root / 'original.json'
        target.write_bytes(b'original retained bytes')
        with mock.patch.object(self.publisher.subprocess, 'run') as command:
            with self.assertRaises(FileExistsError):
                self.publisher.gh('api', 'repos/owner/repo/releases/assets/92', output_path=target)
        command.assert_not_called()
        self.assertEqual(target.read_bytes(), b'original retained bytes')

    def test_explicit_publication_patches_only_the_verified_release_id(self):
        def cli(*args, **kwargs):
            if args[:3] == ('api', '--method', 'PATCH'):
                self.assertEqual(args[3], 'repos/owner/repo/releases/41')
                self.assertEqual(len(self.release['assets']), 17)
                self.assertIn('draft=false', args)
                self.assertIn('make_latest=true', args)
                self.calls.append((args, kwargs))
                return '{}'
            return self.cli(*args, **kwargs)
        with mock.patch.object(self.publisher, 'gh', side_effect=cli):
            self.publisher.publish(self.fixture.output, 'owner/repo', 'v1.0.0',
                                   fixtures.COMMIT, self.fixture.root / 'notes.md', True,
                                   release_id=41)
        self.assertEqual(sum('PATCH' in args for args, _ in self.calls), 1)

    def test_duplicate_on_a_later_page_cannot_be_hidden_by_an_earlier_match(self):
        filler = [{'id': i + 100, 'tag_name': 'v0.0.' + str(i)} for i in range(99)]
        def cli(*args, **kwargs):
            if args == ('api', 'repos/owner/repo/releases?per_page=100&page=1'):
                return json.dumps([self.release] + filler)
            if args == ('api', 'repos/owner/repo/releases?per_page=100&page=2'):
                return json.dumps([dict(self.release, id=42)])
            return self.cli(*args, **kwargs)
        with mock.patch.object(self.publisher, 'gh', side_effect=cli):
            with self.assertRaisesRegex(ValueError, 'Multiple releases'):
                self.publisher.find_release('owner/repo', 'v1.0.0')
        self.assertEqual(self.release['assets'], [])


if __name__ == '__main__':
    unittest.main()
