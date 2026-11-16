# SPDX-License-Identifier: GPL-3.0-or-later
"""Verify the Linux release gate rejects missing or failed packaging jobs."""
import os
from pathlib import Path
import subprocess
import textwrap
import unittest

WORKFLOW = Path(__file__).resolve().parents[2] / '.github/workflows/LinuxDistribution.yml'


class LinuxWorkflowGateTests(unittest.TestCase):
    def test_gate_requires_every_package_job_and_allows_reused_native_artifacts(self):
        workflow = WORKFLOW.read_text()
        gate = workflow.split('  release-gate:\n', 1)[1]
        self.assertIn('needs: [native, build, package, smoke]', gate)
        self.assertIn('always() && !cancelled()', gate)
        script = textwrap.dedent(gate.split('        run: |\n', 1)[1])
        baseline = dict(NATIVE_RESULT='success', USE_EXISTING_NATIVE='false', BUILD_RESULT='success',
                        PACKAGE_RESULT='success', SMOKE_RESULT='success')

        def run(**changes):
            env = dict(os.environ, **(baseline | changes))
            return subprocess.run(['bash', '-e', '-c', script], env=env, capture_output=True).returncode

        self.assertEqual(run(), 0)
        self.assertEqual(run(NATIVE_RESULT='skipped', USE_EXISTING_NATIVE='true'), 0)
        self.assertNotEqual(run(NATIVE_RESULT='skipped'), 0)
        self.assertNotEqual(run(NATIVE_RESULT='failure', USE_EXISTING_NATIVE='true'), 0)
        for job in ('BUILD_RESULT', 'PACKAGE_RESULT', 'SMOKE_RESULT'):
            for result in ('failure', 'skipped', 'cancelled'):
                with self.subTest(job=job, result=result):
                    self.assertNotEqual(run(**{job: result}), 0)


if __name__ == '__main__':
    unittest.main()
