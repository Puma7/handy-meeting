"""Select CI work without confusing omitted package checks with completed proofs."""
import json
import os
from pathlib import Path


def plan(event_name, event, requested):
    """Select package work from an explicit draft state or a validated boolean input."""
    if requested not in ('', 'false', 'true'):
        raise ValueError('CI_PACKAGES must be a boolean workflow input')
    if event_name == 'pull_request':
        draft = event['pull_request']['draft']
        if not isinstance(draft, bool):
            raise ValueError('A pull request must have an explicit draft state')
        packages = not draft
        reason = 'Ready PR: complete candidate checks' if packages else 'Draft PR: source checks only'
    elif event_name in ('push', 'workflow_dispatch', 'release'):
        # Reusable release callers pass true; ordinary main pushes have no input.
        packages = requested == 'true'
        reason = 'Explicit complete candidate checks' if packages else 'Source checks only'
    else:
        raise ValueError('Unsupported CI event: ' + event_name)
    return {'packages': packages, 'reason': reason}


def main():
    """Publish the selected scope without presenting omitted package checks as evidence."""
    result = plan(os.environ['GITHUB_EVENT_NAME'],
                  json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8')),
                  os.environ.get('CI_PACKAGES', ''))
    with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as out:
        out.write('packages=' + str(result['packages']).lower() + '\n')
    summary = os.environ.get('GITHUB_STEP_SUMMARY')
    if summary:
        with open(summary, 'a', encoding='utf-8') as out:
            out.write('### CI scope\n\n' + result['reason'] + '.\n\n')
            if not result['packages']:
                out.write('Installer, native-store, GUI and packaged-release checks are **not executed** '
                          'in this mode. Mark the PR ready for review or run the candidate workflows '
                          'manually before approval. No downloadable candidate is produced.\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
