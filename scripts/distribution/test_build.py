#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise public build planning, literal paths, failure evidence and shared work locks."""
import contextlib
import csv
import fcntl
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import signal
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest import mock

SCRIPT = Path(__file__).with_name('build.py')
spec = importlib.util.spec_from_file_location('distribution_build', SCRIPT)
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class BuildTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory(prefix='whisky-build-test-')
        self.addCleanup(self.folder.cleanup)
        self.root = Path(self.folder.name).resolve()
        self.repo = self.root / 'Wine repo 包 $HOME ;'
        for path in ('scripts/linux/build.py', 'scripts/linux/install-deps.sh',
                     'scripts/ci/build-release.sh', 'configs/crossover-source-lock.json'):
            file = self.repo / path
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('fixture\n')

    def args(self, *args):
        return build.parser().parse_args(['plan', *args, '--wine-repo', str(self.repo),
                                         '--output', str(self.root / 'output 包 $HOME ;')])

    def test_plans_have_no_side_effects_and_invalid_flags_fail(self):
        args = self.args('wine-linux-amd64', '--compile-only')
        value = build.plan(args)
        self.assertFalse(Path(value['output']).exists())
        self.assertIn('compile-only', value['steps'][2]['command'])
        self.assertFalse(any('verify.py' in str(s['command']) for s in value['steps']))
        with self.assertRaises(ValueError):
            build.plan(self.args('whisky-linux-amd64', '--compile-only'))
        with self.assertRaises(ValueError):
            build.plan(self.args('whisky-linux-arm64', '--compile-only'))
        with self.assertRaises(ValueError):
            build.plan(self.args('whisky-macos-gpui-arm64', '--version', '1.2;touch /tmp/x'))

    def test_default_output_is_outside_checkout(self):
        with mock.patch.dict(os.environ):
            os.environ.pop('MALTORIA_LUNAHOOK_ARTIFACTS', None)
            for target in ('whisky-linux-amd64', 'whisky-macos-gpui-arm64'):
                args = build.parser().parse_args([
                    'plan', target, '--wine-repo', str(self.repo)])
                value = build.plan(args)
                self.assertTrue(value['lunahookBuild'])
                self.assertFalse(Path(value['output']).is_relative_to(build.ROOT))
                lunahook_step = next(step for step in value['steps']
                                     if 'build-lunahook.py' in step['command'][1])
                output = Path(lunahook_step['command'][lunahook_step['command'].index('--output') + 1])
                self.assertFalse(output.is_relative_to(build.ROOT))

    def test_default_output_root_falls_back_to_user_cache(self):
        cache = self.root / 'xdg-cache'
        expected = cache / 'maltoria-build' / 'distribution'
        with mock.patch.dict(os.environ):
            os.environ.pop('MALTORIA_LUNAHOOK_ARTIFACTS', None)
            with mock.patch.dict(os.environ, {'XDG_CACHE_HOME': str(cache)}), \
                    mock.patch.object(build.os, 'access', return_value=False):
                self.assertEqual(expected, build.default_output_root())
                args = build.parser().parse_args([
                    'plan', 'whisky-linux-amd64', '--wine-repo', str(self.repo)])
                value = build.plan(args)
                self.assertTrue(Path(value['output']).is_relative_to(expected))

    def test_local_lunahook_build_rejects_output_inside_checkout(self):
        with mock.patch.dict(os.environ):
            os.environ.pop('MALTORIA_LUNAHOOK_ARTIFACTS', None)
            output = str(build.ROOT / 'build/x')
            args = build.parser().parse_args([
                'plan', 'whisky-linux-amd64', '--wine-repo', str(self.repo), '--output', output])
            with self.assertRaisesRegex(ValueError, '源码检出目录之外'):
                build.plan(args)

            args = build.parser().parse_args([
                'plan', 'whisky-linux-amd64', '--wine-repo', str(self.repo), '--output', output,
                '--without-lunahook'])
            self.assertEqual(str(Path(output).resolve()), build.plan(args)['output'])

    def test_runtime_validation_follows_packaging_and_recipe_cache_changes(self):
        before = build.plan(self.args('wine-linux-amd64'))
        commands = [s['command'] for s in before['steps']]
        self.assertIn('runtime', commands[3])
        self.assertIn('--runtime', commands[-1])
        self.assertTrue(commands[-1][-1].endswith('artifacts/wine-linux-amd64'))
        (self.repo / 'configs/crossover-source-lock.json').write_text('new recipe')
        after = build.plan(self.args('wine-linux-amd64'))
        self.assertNotEqual(before['workDirectory'], after['workDirectory'])

    def test_docker_mounts_are_literal_and_context_is_minimal(self):
        args = self.args('wine-linux-amd64', '--executor', 'docker', '--compile-only')
        value = build.plan(args)
        command = value['steps'][1]['command']
        self.assertIn(f'type=bind,src={self.repo},dst=/wine,readonly', command)
        self.assertIn('linux/amd64', command)
        self.assertIn('/output/task', command)
        self.assertEqual(2, len(value['contextFiles']))
        self.assertEqual(str(Path(value['output']) / 'docker-context'), value['steps'][0]['command'][-1])
        self.assertEqual('compile-only', value['acceptance'])
        self.assertFalse(Path(value['output']).exists())

    def test_docker_mount_csv_preserves_commas_and_quotes(self):
        source = self.root / 'source, "quoted" 包 $HOME'
        fields = next(csv.reader([build.mount(source, '/wine', readonly=True)]))
        self.assertEqual(['type=bind', 'src=' + str(source), 'dst=/wine', 'readonly'], fields)

    def test_linux_formats_share_one_payload_and_forward_version_to_docker(self):
        value = build.plan(self.args('whisky-linux-amd64', '--version', '0.2.0'))
        command = value['steps'][-1]['command']
        self.assertIn('package-linux.py', Path(command[1]).name)
        self.assertEqual(['deb', 'rpm', 'appimage'], command[command.index('--formats') + 1:])
        self.assertEqual('maltoria-linux-x86_64.tar.gz', Path(command[command.index('--archive') + 1]).name)
        raw = build.plan(self.args('whisky-linux-amd64', '--formats', 'tar'))
        # tar-only skips packaging; the plan is the two local LunaHook build
        # steps plus release and audit.
        self.assertEqual(4, len(raw['steps']))
        self.assertIn('build-lunahook.py', raw['steps'][0]['command'][1])
        self.assertEqual('local-build', raw['lunahook'])
        self.assertTrue(raw['lunahookBuild'])
        docker = build.plan(self.args('whisky-linux-amd64', '--version', '0.2.0', '--formats', 'deb', '--executor', 'docker'))
        command = docker['steps'][-1]['command']
        self.assertIn('linux/amd64', docker['steps'][0]['command'])
        self.assertIn('linux/amd64', command)
        self.assertIn('0.2.0', command)
        self.assertEqual(['deb'], command[command.index('--formats') + 1:])

    def test_linux_arm64_target_plans_aarch64_archive_and_docker_platform(self):
        self.assertEqual('arm64', build.TARGETS['whisky-linux-arm64'][1])
        value = build.plan(self.args('whisky-linux-arm64', '--version', '0.3.0', '--formats', 'deb'))
        command = value['steps'][-1]['command']
        self.assertEqual('maltoria-linux-aarch64.tar.gz', Path(command[command.index('--archive') + 1]).name)
        self.assertEqual(['deb'], command[command.index('--formats') + 1:])
        docker = build.plan(self.args('whisky-linux-arm64', '--version', '0.3.0', '--formats', 'deb', '--executor', 'docker'))
        build_cmd = docker['steps'][0]['command']
        run_cmd = docker['steps'][1]['command']
        self.assertIn('linux/arm64', build_cmd)
        self.assertTrue(any(':ubuntu24-arm64-' in token for token in build_cmd))
        self.assertIn('linux/arm64', run_cmd)
        self.assertIn('whisky-linux-arm64', run_cmd)
        with self.assertRaises(ValueError):
            build.plan(self.args('whisky-linux-arm64', '--install'))

    def test_macos_gpui_plan_adds_package_step_when_version_provided(self):
        self.assertEqual([name for name in build.TARGETS if name.startswith("whisky-macos-")],
                         ["whisky-macos-gpui-arm64"])
        dev = build.plan(self.args('whisky-macos-gpui-arm64'))
        # Without --lunahook the app plan builds the payload locally first.
        self.assertEqual(3, len(dev['steps']))
        self.assertIn('build-lunahook.py', dev['steps'][0]['command'][1])
        self.assertIn('package-native-sources.py', dev['steps'][1]['command'][1])
        self.assertIn('package-app.sh', dev['steps'][2]['command'][1])
        self.assertNotIn('WHISKY_VERSION', dev['environment'])
        install = build.plan(self.args('whisky-macos-gpui-arm64', '--install'))
        self.assertEqual('install-macos.py', Path(install['steps'][-1]['command'][1]).name)
        self.assertIn('--remove-source', install['steps'][-1]['command'])
        with self.assertRaises(ValueError):
            build.plan(self.args('whisky-linux-amd64', '--install'))

        release = build.plan(self.args('whisky-macos-gpui-arm64', '--version', '0.4.0', '--build-number', '7'))
        self.assertEqual(4, len(release['steps']))
        self.assertEqual('0.4.0', release['environment']['WHISKY_VERSION'])
        self.assertEqual('7', release['environment']['BUILD_NUMBER'])
        self.assertIn('package-macos.py', release['steps'][3]['command'][1])
        self.assertIn('--version', release['steps'][3]['command'])
        self.assertIn('0.4.0', release['steps'][3]['command'])
        with self.assertRaises(ValueError):
            build.plan(self.args('whisky-macos-gpui-arm64', '--version', 'invalid_version'))

    def job(self, commands, **extra):
        value = dict(target='whisky-linux-amd64', executor='native', output=str(self.root / 'output'),
                     environment={}, steps=[dict(name='fixture', cwd=str(self.root), command=c) for c in commands])
        value.update(extra)
        return value

    def run_job(self, value):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            build.execute(value)

    def test_literal_paths_and_arguments_survive_real_process_execution(self):
        destination = self.root / 'result 包 $HOME ; $(touch BAD)'
        literal = '文字 " ; $HOME $(touch BAD)'
        job = self.job([[sys.executable, '-c',
            'from pathlib import Path; import sys; Path(sys.argv[1]).write_text(sys.argv[2])',
            str(destination), literal]])
        self.run_job(job)
        self.assertEqual(literal, destination.read_text())
        self.assertFalse((self.root / 'BAD').exists())
        report = json.loads((Path(job['output']) / 'build-report.json').read_text())
        self.assertEqual('passed', report['status'])
        self.assertEqual(0, report['results'][0]['exitCode'])

    def test_failure_preserves_evidence_and_stops_later_steps(self):
        marker = self.root / 'should-stay-absent'
        job = self.job([[sys.executable, '-c', 'print("build failure evidence"); raise SystemExit(7)'],
                        [sys.executable, '-c', f'from pathlib import Path; Path({str(marker)!r}).touch()']])
        with self.assertRaises(subprocess.CalledProcessError):
            self.run_job(job)
        self.assertFalse(marker.exists())
        report = json.loads((Path(job['output']) / 'build-report.json').read_text())
        self.assertEqual('failed', report['status'])
        self.assertEqual(7, report['results'][0]['exitCode'])
        self.assertIn('build failure evidence', (Path(job['output']) / '01.log').read_text())
        with self.assertRaises(FileExistsError):
            self.run_job(job)
        self.assertIn('build failure evidence', (Path(job['output']) / '01.log').read_text())

    def test_wine_work_lock_blocks_concurrent_build_before_launch(self):
        work = self.root / 'cache/work'
        work.parent.mkdir()
        with (work.parent / 'work.build.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            job = self.job([[sys.executable, '-c', 'raise SystemExit(99)']],
                           target='wine-linux-amd64', workDirectory=str(work))
            with self.assertRaisesRegex(ValueError, '正在构建'):
                self.run_job(job)
        report = json.loads((Path(job['output']) / 'build-report.json').read_text())
        self.assertEqual('failed', report['status'])
        self.assertEqual([], report['results'])

    def test_lunahook_flag_env_app_resolution_docker_mount_and_run_guard(self):
        # Build a valid LunaHook directory fixture nested inside a fake .app bundle.
        app_bundle = self.root / 'Existing.app'
        payload = app_bundle / 'Contents/Resources/LunaHook'
        (payload / 'x86').mkdir(parents=True)
        (payload / 'x64').mkdir(parents=True)
        (payload / 'sources/Maltoria').mkdir(parents=True)
        (payload / 'sources/Maltoria/SOURCE-COMMIT.txt').write_text('abc123\n')

        # Passing an .app bundle resolves to Contents/Resources/LunaHook and enforces MALTORIA_REQUIRE_LUNAHOOK=1.
        mac_plan = build.plan(self.args('whisky-macos-gpui-arm64', '--lunahook', str(app_bundle)))
        self.assertEqual(str(payload), mac_plan['lunahook'])
        self.assertEqual(str(payload), mac_plan['environment']['MALTORIA_LUNAHOOK_ARTIFACTS'])
        self.assertEqual('1', mac_plan['environment']['MALTORIA_REQUIRE_LUNAHOOK'])

        # Docker Linux build mounts the LunaHook directory read-only and forwards --lunahook /lunahook.
        docker_plan = build.plan(self.args(
            'whisky-linux-amd64', '--executor', 'docker', '--lunahook', str(payload)))
        run_cmd = docker_plan['steps'][1]['command']
        self.assertIn(f'type=bind,src={payload},dst=/lunahook,readonly', run_cmd)
        self.assertIn('/lunahook', run_cmd)

        # --without-lunahook explicitly clears MALTORIA_REQUIRE_LUNAHOOK.
        skipped = build.plan(self.args('whisky-macos-gpui-arm64', '--without-lunahook'))
        self.assertTrue(skipped['withoutLunahook'])
        self.assertEqual('0', skipped['environment']['MALTORIA_REQUIRE_LUNAHOOK'])

        # Invalid combinations fail fast.
        with self.assertRaises(ValueError):
            build.plan(self.args('whisky-macos-gpui-arm64', '--lunahook', str(payload), '--without-lunahook'))
        with self.assertRaises(ValueError):
            build.plan(self.args('wine-linux-amd64', '--without-lunahook'))
        with self.assertRaises(ValueError):
            build.plan(self.args('whisky-macos-gpui-arm64', '--lunahook', str(self.root / 'missing')))

        # The native executor defaults to a local LunaHook build instead of failing.
        native = build.plan(self.args('whisky-macos-gpui-arm64'))
        self.assertEqual('local-build', native['lunahook'])
        self.assertTrue(native['lunahookBuild'])
        self.assertIn('MALTORIA_LUNAHOOK_ARTIFACTS', native['environment'])

        # The Docker executor has no cross toolchain in its image, so running
        # without --lunahook / env / --without-lunahook still exits 1 with a
        # helpful prompt.
        env_clean = {k: v for k, v in os.environ.items() if k != 'MALTORIA_LUNAHOOK_ARTIFACTS'}
        result = subprocess.run(
            [sys.executable, str(SCRIPT), 'run', 'whisky-linux-amd64',
             '--executor', 'docker',
             '--output', str(self.root / 'blocked-out')],
            env=env_clean, capture_output=True, text=True)
        self.assertEqual(1, result.returncode)
        self.assertIn('--lunahook', result.stderr)
        self.assertIn('--without-lunahook', result.stderr)
        self.assertFalse((self.root / 'blocked-out').exists())

    def test_doctor_toolchain_skips_xwin_when_sysroot_cached(self):
        builder_module = build.lunahook_builder()
        xwin_dir = self.root / 'xwin'

        def populate_sysroot():
            for relative in ('crt/include', 'sdk/include', 'crt/lib/x86', 'crt/lib/x86_64',
                             'sdk/lib/um/x86', 'sdk/lib/um/x86_64',
                             'sdk/lib/ucrt/x86', 'sdk/lib/ucrt/x86_64'):
                (xwin_dir / relative).mkdir(parents=True, exist_ok=True)

        def fake_builder(has_xwin=False):
            return SimpleNamespace(
                locate_tool=lambda name: Path('/fake/' + name) if name != 'xwin' or has_xwin else None,
                default_xwin_dir=lambda: xwin_dir,
                xwin_sysroot_ready=builder_module.xwin_sysroot_ready,
            )

        populate_sysroot()
        ready = build.lunahook_toolchain_check(fake_builder())
        self.assertTrue(ready['passed'])

        (xwin_dir / 'sdk/lib/ucrt/x86').rmdir()
        missing = build.lunahook_toolchain_check(fake_builder())
        self.assertFalse(missing['passed'])
        self.assertIn('xwin', missing['name'])

        available = build.lunahook_toolchain_check(fake_builder(has_xwin=True))
        self.assertTrue(available['passed'])

    def test_locate_tool_checks_versioned_homebrew_llvm(self):
        builder = build.lunahook_builder()
        homebrew_opt = Path('/opt/homebrew/opt')
        llvm19 = homebrew_opt / 'llvm@19/bin'
        llvm22 = homebrew_opt / 'llvm@22/bin'

        def fake_glob(path, pattern):
            if path == homebrew_opt and pattern == 'llvm@*/bin':
                return [llvm19, llvm22]
            return []

        with mock.patch.object(builder.Path, 'glob', fake_glob), \
                mock.patch.object(builder.Path, 'is_dir', lambda _path: True):
            directories = builder.llvm_search_dirs()

        self.assertLess(directories.index(llvm22), directories.index(llvm19))
        self.assertIn(llvm22, directories)
        self.assertIn(llvm19, directories)

    def test_locate_tool_uses_llvm_dir_without_side_effects(self):
        builder = build.lunahook_builder()
        llvm_dir = self.root / 'llvm'
        llvm_dir.mkdir()
        tool = llvm_dir / 'maltoria-fake-tool'
        tool.write_text('fixture\n')
        tool.chmod(0o755)
        with mock.patch.dict(os.environ, {'LLVM_DIR': str(llvm_dir)}):
            self.assertEqual(tool, builder.locate_tool('maltoria-fake-tool'))
            self.assertIsNone(builder.locate_tool('definitely-missing-tool-xyz'))

    def test_menu_docker_hides_local_build(self):
        payload = self.root / 'payload'
        (payload / 'x86').mkdir(parents=True)
        (payload / 'x64').mkdir()
        (payload / 'sources/Maltoria').mkdir(parents=True)
        (payload / 'sources/Maltoria/SOURCE-COMMIT.txt').write_text('abc123\n')

        def args(executor='docker'):
            return build.parser().parse_args([
                'plan', 'whisky-linux-amd64', '--executor', executor,
                '--wine-repo', str(self.repo), '--output', str(self.root / f'{executor}-menu-output')])

        def choose(value, detected=None, expect_error=False):
            prompts = []
            with mock.patch.object(build, 'discover_local_lunahook', return_value=detected), \
                    mock.patch('builtins.input', side_effect=lambda prompt: prompts.append(prompt) or value):
                if expect_error:
                    with self.assertRaises(ValueError):
                        build.choose_lunahook(args_under_test)
                else:
                    build.choose_lunahook(args_under_test)
            return prompts

        args_under_test = args()
        prompts = choose('', expect_error=True)
        self.assertNotIn('本机交叉编译', prompts[0])

        args_under_test = args()
        prompts = choose('skip')
        self.assertTrue(args_under_test.without_lunahook)
        self.assertNotIn('本机交叉编译', prompts[0])

        args_under_test = args()
        prompts = choose(str(payload))
        self.assertEqual(payload.resolve(), args_under_test.lunahook)
        self.assertNotIn('本机交叉编译', prompts[0])
        docker_plan = build.plan(args_under_test)
        run_command = docker_plan['steps'][1]['command']
        self.assertIn(f'type=bind,src={payload.resolve()},dst=/lunahook,readonly', run_command)
        self.assertIn('/lunahook', run_command)

        args_under_test = args()
        prompts = choose('', detected=payload)
        self.assertEqual(payload.resolve(), args_under_test.lunahook)
        self.assertNotIn('本机交叉编译', prompts[0])

        args_under_test = args('native')
        prompts = choose('', detected=None)
        self.assertIsNone(args_under_test.lunahook)
        self.assertFalse(args_under_test.without_lunahook)
        self.assertIn('本机交叉编译', prompts[0])

    def test_cancellation_terminates_child_and_keeps_report(self):
        marker = self.root / 'child.pid'
        job = self.job([[sys.executable, '-c',
            'import os,time; from pathlib import Path; '
            f'Path({str(marker)!r}).write_text(str(os.getpid())); time.sleep(60)']])
        bootstrap = ('import importlib.util,json,sys; '
            's=importlib.util.spec_from_file_location("builder",sys.argv[1]); '
            'm=importlib.util.module_from_spec(s); s.loader.exec_module(m); '
            'm.execute(json.loads(sys.argv[2]))')
        process = subprocess.Popen([sys.executable, '-c', bootstrap, str(SCRIPT), json.dumps(job)],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.addCleanup(lambda: process.kill() if process.poll() is None else None)
        deadline = time.monotonic() + 10
        while not marker.exists() and process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertTrue(marker.exists())
        child = int(marker.read_text())
        process.send_signal(signal.SIGINT)
        process.wait(timeout=15)
        with self.assertRaises(ProcessLookupError):
            os.kill(child, 0)
        report = json.loads((Path(job['output']) / 'build-report.json').read_text())
        self.assertEqual('cancelled', report['status'])


if __name__ == '__main__':
    unittest.main()
