#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Unified local build menu and CI command line for Maltoria and its Wine recipes."""
import argparse
import csv
import fcntl
import importlib.util
import io
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import signal
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
TARGETS = {
    'whisky-macos-gpui-arm64': ('Darwin', 'arm64', 'GPUI · ad-hoc ZIP · 预发布', ('swift', 'cargo', 'iconutil', 'codesign', 'ditto')),
    'whisky-linux-amd64': ('Linux', 'x86_64', 'GPUI · Ubuntu 24.04 · tar.gz', ('swift', 'cargo', 'clang', 'patchelf', 'pkg-config')),
    'whisky-linux-arm64': ('Linux', 'arm64', 'GPUI · Ubuntu 24.04 · tar.gz', ('swift', 'cargo', 'clang', 'patchelf', 'pkg-config')),
    'wine-macos-arm64': ('Darwin', 'arm64', 'Apple Silicon 主机 · x86_64 Wine/Rosetta · Libraries', ('cmake', 'pkg-config', 'xcrun')),
    'wine-linux-amd64': ('Linux', 'x86_64', 'Ubuntu 24.04 · new WoW64 · tar.gz', ('cc', 'make', 'patchelf', 'pkg-config', 'git')),
}
LINUX_APP_TARGETS = ('whisky-linux-amd64', 'whisky-linux-arm64')
APP_TARGETS = ('whisky-macos-gpui-arm64',) + LINUX_APP_TARGETS
LINUX_DOCKER_ARCH = {'x86_64': 'amd64', 'arm64': 'arm64'}
LINUX_RELEASE_ARCH = {'x86_64': 'x86_64', 'arm64': 'aarch64'}
LUNAHOOK_TOOLS = ('cmake', 'ninja', 'clang-cl', 'lld-link', 'llvm-lib', 'llvm-rc', 'llvm-mt', 'mcs')
LUNAHOOK_ARCHES = ('x86', 'x64')


def default_output_root():
    # scripts/build-lunahook.py 拒绝源码树内的输出目录，默认任务目录放在检出目录之外；
    # 检出目录的上级不可写时退回用户缓存目录。
    sibling = ROOT.parent / f'{ROOT.name}-build'
    if os.access(sibling if sibling.exists() else ROOT.parent, os.W_OK):
        return sibling / 'distribution'
    cache = Path(os.environ.get('XDG_CACHE_HOME') or Path.home() / '.cache')
    return cache / 'maltoria-build' / 'distribution'


def is_inside_checkout(path):
    return Path(path).resolve().is_relative_to(ROOT)


def lunahook_builder():
    spec = importlib.util.spec_from_file_location('maltoria_build_lunahook', ROOT / 'scripts/build-lunahook.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def is_valid_lunahook_dir(path):
    path = Path(path)
    return (path.is_dir()
            and (path / 'x86').is_dir()
            and (path / 'x64').is_dir()
            and (path / 'sources/Maltoria/SOURCE-COMMIT.txt').is_file())


def resolve_lunahook_dir(raw):
    path = Path(raw).expanduser().resolve()
    nested = path / 'Contents/Resources/LunaHook'
    if not is_valid_lunahook_dir(path) and is_valid_lunahook_dir(nested):
        path = nested
    if not path.is_dir():
        raise ValueError(f'LunaHook 载荷目录不存在：{path}')
    missing = [rel for rel in ('x86', 'x64', 'sources/Maltoria/SOURCE-COMMIT.txt') if not (path / rel).exists()]
    if missing:
        raise ValueError(
            f'LunaHook 载荷目录不完整（{path}），缺少：{", ".join(missing)}。'
            '请指定包含 x86/、x64/ 与 sources/Maltoria/ 的 NativeLunaHook 产物目录或已打包的 LunaHook 目录。'
        )
    return path


def discover_local_lunahook():
    candidates = []
    if env_path := os.environ.get('MALTORIA_LUNAHOOK_ARTIFACTS', '').strip():
        candidates.append(Path(env_path))
    candidates.extend([
        ROOT / 'dist/Maltoria.app/Contents/Resources/LunaHook',
        Path('/Applications/Maltoria.app/Contents/Resources/LunaHook'),
    ])
    for candidate in candidates:
        try:
            return resolve_lunahook_dir(candidate)
        except ValueError:
            continue
    return None


def missing_lunahook_message(target):
    lines = [
        '未指定 LunaHook 文本捕获载荷目录，直接构建的应用将无法使用游戏内文本捕获（capture_payload_unavailable）。',
        '请使用以下方式之一继续：',
        '  1. 本机交叉编译（默认；需要 clang-cl/lld-link/llvm-*/ninja/mono mcs/xwin）：',
        f'     ./build.sh run {target}    # 原生执行器下不传 --lunahook 即自动本机构建',
        f'  2. 指定 LunaHook 目录：./build.sh run {target} --lunahook /path/to/LunaHook',
        '     （也支持直接传入已包含 LunaHook 的 Maltoria.app，或设置环境变量 MALTORIA_LUNAHOOK_ARTIFACTS）',
        f'  3. 显式跳过 LunaHook（仅本地调试非捕获功能）：./build.sh run {target} --without-lunahook',
    ]
    if detected := discover_local_lunahook():
        lines.append(f'提示：检测到本机现成 LunaHook 载荷，可直接复用：\n  ./build.sh run {target} --lunahook {shlex.quote(str(detected))}')
    return '\n'.join(lines)


def positive(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError('请输入正整数')
    return number


def parser():
    result = argparse.ArgumentParser(description=__doc__, epilog='运行 ./build.sh menu 打开交互菜单。')
    result.add_argument('action', choices=('menu', 'list', 'doctor', 'plan', 'run'), nargs='?', default='menu')
    result.add_argument('target', choices=TARGETS, nargs='?')
    result.add_argument('--install', action='store_true', help='构建后替换 /Applications/Maltoria.app（macOS GPUI）')
    result.add_argument('--version', help='应用或 Linux 安装包版本 X.Y.Z')
    result.add_argument('--formats', nargs='+', choices=('tar', 'deb', 'rpm', 'appimage'),
                        help='Linux 应用格式；默认 tar deb rpm appimage')
    result.add_argument('--build-number', type=positive, default=1,
                        help='应用 build number；macOS Wine 的独立运行库版本计数器，默认 1')
    result.add_argument('--jobs', type=positive, default=4)
    result.add_argument('--wine-repo', type=Path, default=Path(os.environ.get(
        'WHISKY_WINE_REPO', str(ROOT.parent / 'ge-proton-macos-wine-poc'))))
    result.add_argument('--output', type=Path, help='本次任务的全新输出目录')
    result.add_argument('--work-dir', type=Path, help='Linux Wine 增量构建目录；macOS Wine 使用全新目录')
    result.add_argument('--archive-cache', type=Path, help='Wine 已校验下载的缓存目录')
    result.add_argument('--compile-only', action='store_true', help='Linux Wine 编译/静态校验；产物记录 compile-only')
    result.add_argument('--lunahook', type=Path,
                        help='LunaHook 载荷目录（含 x86/、x64/、sources/，或已包含它的 Maltoria.app）；未指定时默认本机交叉编译（Docker 执行器除外），也可用 MALTORIA_LUNAHOOK_ARTIFACTS 提供')
    result.add_argument('--without-lunahook', action='store_true',
                        help='显式跳过 LunaHook 捕获载荷（应用内无法使用游戏文本捕获）')
    result.add_argument('--executor', choices=('native', 'docker'), default='native')
    result.add_argument('--json', action='store_true', help='计划或环境检查输出 JSON')
    return result


def revision(folder):
    try:
        return subprocess.check_output(['git', '-C', str(folder), 'rev-parse', 'HEAD'],
                                       text=True, stderr=subprocess.DEVNULL, timeout=5).strip()
    except (OSError, subprocess.SubprocessError):
        return None


def recipe_key(repo):
    """Cache only exact recipes; preserve old work trees when recipe inputs change."""
    digest = hashlib.sha256()
    for directory in ('configs', 'patches', 'scripts'):
        for path in sorted((repo / directory).rglob('*')):
            if path.is_file() and '.pyc' != path.suffix and '__pycache__' not in path.parts:
                digest.update(str(path.relative_to(repo)).encode())
                digest.update(path.read_bytes())
    return digest.hexdigest()[:16]


def mount(source, destination, readonly=False):
    output = io.StringIO()
    csv.writer(output, lineterminator='').writerow(
        ['type=bind', 'src=' + str(source), 'dst=' + destination] + (['readonly'] if readonly else []))
    return output.getvalue()


def plan(args):
    if not args.target:
        raise ValueError('请选择构建目标，例如 ./build.sh plan whisky-linux-amd64')
    if args.install and (args.target != 'whisky-macos-gpui-arm64' or args.executor != 'native'):
        raise ValueError('--install 适用于原生 macOS GPUI 构建')
    if args.formats and args.target not in LINUX_APP_TARGETS:
        raise ValueError('--formats 适用于 whisky-linux-amd64 / whisky-linux-arm64')
    is_wine = args.target.startswith('wine-')
    is_app = args.target in APP_TARGETS
    if args.lunahook and args.without_lunahook:
        raise ValueError('--lunahook 与 --without-lunahook 不能同时使用')
    if (args.lunahook or args.without_lunahook) and not is_app:
        raise ValueError('--lunahook 与 --without-lunahook 仅适用于应用构建目标')
    raw_lunahook = args.lunahook
    if raw_lunahook is None and is_app and not args.without_lunahook:
        env_lunahook = os.environ.get('MALTORIA_LUNAHOOK_ARTIFACTS', '').strip()
        if env_lunahook:
            raw_lunahook = Path(env_lunahook)
    lunahook_dir = resolve_lunahook_dir(raw_lunahook) if raw_lunahook is not None else None
    # 未指定载荷时默认本机交叉编译（无需 Windows）；Docker 执行器的镜像没有
    # 该工具链，仍要求显式 --lunahook 或 --without-lunahook。
    lunahook_build = (is_app and lunahook_dir is None and not args.without_lunahook
                      and args.executor == 'native')
    if args.compile_only and args.target != 'wine-linux-amd64':
        raise ValueError('--compile-only 适用于 wine-linux-amd64')
    if (args.work_dir or args.archive_cache) and not is_wine:
        raise ValueError('--work-dir 与 --archive-cache 适用于 Wine 目标')
    if args.archive_cache and args.target == 'wine-macos-arm64':
        raise ValueError('--archive-cache 适用于 Linux Wine；macOS 缓存由其构建脚本管理')
    repo = args.wine_repo.expanduser().resolve() if is_wine else ROOT
    required = 'scripts/linux/build.py' if args.target == 'wine-linux-amd64' else 'scripts/ci/build-release.sh'
    if is_wine and not (repo / required).is_file():
        raise ValueError('Wine 仓库缺少配方：' + str(repo / required))
    output = (args.output or default_output_root() / args.target /
              (datetime.now().strftime('%Y%m%d-%H%M%S') + '-' + uuid.uuid4().hex[:6])).expanduser().resolve()
    if lunahook_build and is_inside_checkout(output):
        raise ValueError(f'本机交叉编译 LunaHook 时输出目录必须位于源码检出目录之外：{output}；'
                         '请改用检出目录外的 --output，或通过 --lunahook / --without-lunahook 跳过本机构建')
    env = {'WHISKY_BUILD_JOBS': str(args.jobs), 'POC_JOBS': str(args.jobs)}
    if is_app and lunahook_dir is not None:
        env['MALTORIA_LUNAHOOK_ARTIFACTS'] = str(lunahook_dir)
        env['MALTORIA_REQUIRE_LUNAHOOK'] = '1'
    elif is_app and args.without_lunahook:
        env['MALTORIA_LUNAHOOK_ARTIFACTS'] = ''
        env['MALTORIA_REQUIRE_LUNAHOOK'] = '0'
    elif lunahook_build:
        native_out = output / 'lunahook-native'
        env['MALTORIA_LUNAHOOK_ARTIFACTS'] = str(native_out)
        env['MALTORIA_REQUIRE_LUNAHOOK'] = '1'
    steps = []

    def step(name, command):
        steps.append(dict(name=name, command=list(map(str, command)), cwd=str(repo)))

    if lunahook_build:
        step('LunaHook 本机交叉编译（clang-cl/lld-link/xwin，x86+x64）',
             ['python3', ROOT / 'scripts/build-lunahook.py', '--architecture', 'all',
              '--output', output / 'lunahook-native', '--accept-xwin-license'])
        step('LunaHook 源码与合规包',
             ['python3', ROOT / 'scripts/package-native-sources.py',
              '--output', output / 'lunahook-native/sources'])

    work = None
    if args.target == 'whisky-macos-gpui-arm64':
        app_dir = output / 'artifacts/Maltoria.app'
        env['WHISKY_GPUI_APP_DIR'] = str(app_dir)
        if args.version:
            if not re.fullmatch(r'\d+\.\d+\.\d+', args.version):
                raise ValueError('请提供 --version X.Y.Z')
            env['WHISKY_VERSION'] = args.version
            env['BUILD_NUMBER'] = str(args.build_number)
        step('GPUI 应用构建与封装', ['bash', repo / 'apps/whisky-gpui/tools/package-app.sh'])
        if args.version:
            packages_dir = output / 'packages'
            step('macOS Release 资产打包与校验和', ['python3', ROOT / 'scripts/distribution/package-macos.py',
                 '--app', app_dir, '--version', args.version, '--build-number', str(args.build_number),
                 '--output', packages_dir])
        if args.install:
            step('部署到 /Applications/Maltoria.app', ['python3', ROOT / 'scripts/distribution/install-macos.py',
                 '--remove-source', app_dir])
    elif args.target in LINUX_APP_TARGETS:
        version = args.version or '0.1.0'
        if not re.fullmatch(r'\d+\.\d+\.\d+', version):
            raise ValueError('请提供 --version X.Y.Z')
        env['WHISKY_LINUX_VERSION'] = version
        env['WHISKY_LINUX_RELEASE_OUTPUT'] = str(output / 'artifacts')
        step('Linux Release 与源码、许可证打包', ['bash', repo / 'apps/whisky-gpui/tools/build-linux-release.sh'])
        step('glibc 基线和 ELF 依赖报告', ['python3', ROOT / 'scripts/distribution/audit-elf.py',
             output / 'artifacts/Maltoria', '--output', output / 'elf-abi.json'])
        formats = [name for name in (args.formats or ['tar', 'deb', 'rpm', 'appimage']) if name != 'tar']
        if formats:
            archive_arch = LINUX_RELEASE_ARCH[TARGETS[args.target][1]]
            step('Linux 安装包与 AppImage', ['python3', ROOT / 'scripts/distribution/package-linux.py',
                '--archive', output / f'artifacts/maltoria-linux-{archive_arch}.tar.gz', '--version', version,
                '--output', output / 'packages', '--formats', *formats])
    elif args.target == 'wine-macos-arm64':
        work = (args.work_dir or output / 'work').expanduser().resolve()
        env.update(POC_WORK_DIR=str(work), POC_RELEASE_DIR=str(output / 'artifacts'),
                   GITHUB_RUN_NUMBER=str(args.build_number))
        step('Wine 依赖、CrossOver/GE 与 Libraries 打包', ['bash', repo / 'scripts/ci/build-release.sh'])
    else:
        work = (args.work_dir or ROOT / 'build/distribution-cache/wine-linux-amd64' /
                recipe_key(repo)).expanduser().resolve()
        command = ['python3', repo / 'scripts/linux/build.py', 'all', '--work-dir', work, '--jobs', args.jobs]
        if args.archive_cache:
            command += ['--archive-cache', args.archive_cache.expanduser().resolve()]
        step('Wine 源码、依赖与增量编译', command)
        step('构建回归探针', ['python3', repo / 'scripts/linux/build-probes.py', '--work-dir', work])
        if not args.compile_only:
            step('隔离 Wine 运行回归', ['xvfb-run', '-a', '-s', '-screen 0 1280x720x24', 'python3',
                                    repo / 'scripts/linux/verify.py', '--work-dir', work])
        step('Wine 校验、源码与发布包', ['python3', repo / 'scripts/linux/package.py', '--work-dir', work,
             '--output', output / 'artifacts', '--validation-mode', 'compile-only' if args.compile_only else 'runtime'])
        step('glibc 基线和 Wine ELF 依赖报告', ['python3', ROOT / 'scripts/distribution/audit-elf.py',
             output / 'artifacts/wine-linux-amd64', '--output', output / 'elf-abi.json'])
        if not args.compile_only:
            step('打包后 Wine 回归', ['xvfb-run', '-a', '-s', '-screen 0 1280x720x24', 'python3',
                 repo / 'scripts/linux/verify.py', '--work-dir', work, '--runtime', output / 'artifacts/wine-linux-amd64'])
    result = dict(formatVersion=1, target=args.target, description=TARGETS[args.target][2],
                  executor=args.executor, source=str(repo), commit=revision(repo), output=str(output),
                  workDirectory=str(work) if work else None,
                  lunahook=str(lunahook_dir) if lunahook_dir else ('local-build' if lunahook_build else None),
                  lunahookBuild=lunahook_build,
                  withoutLunahook=bool(args.without_lunahook) if is_app else None,
                  environment=env, steps=steps,
                  acceptance='compile-only' if args.compile_only else 'recipe-defined', publishes=False)
    if args.executor == 'docker':
        if TARGETS[args.target][0] != 'Linux':
            raise ValueError('Docker 执行器支持 Linux 目标；macOS 目标使用 Apple 主机')
        if args.work_dir or args.archive_cache:
            raise ValueError('Docker 执行器在持久化 /cache 下自动管理 Wine 工作目录和下载缓存')
        docker_arch = LINUX_DOCKER_ARCH[TARGETS[args.target][1]]
        docker_platform = 'linux/' + docker_arch
        dockerfile = ROOT / 'scripts/distribution' / ('wine.Dockerfile' if is_wine else 'app.Dockerfile')
        dependency_script = repo / ('scripts/linux/install-deps.sh' if is_wine else
                                    'apps/whisky-gpui/tools/install-linux-deps.sh')
        image_key = hashlib.sha256(dockerfile.read_bytes() + dependency_script.read_bytes()).hexdigest()[:16]
        image = 'whisky-builder-' + ('wine' if is_wine else 'app') + f':ubuntu24-{docker_arch}-' + image_key
        cache = ROOT / 'build/distribution-cache/docker' / args.target
        inner = ['python3', '/workspace/scripts/distribution/build.py', 'run', args.target,
                 '--output', '/output/task', '--jobs', str(args.jobs), '--wine-repo', '/wine']
        if args.compile_only:
            inner += ['--compile-only']
        if args.version:
            inner += ['--version', args.version]
        if args.formats:
            inner += ['--formats', *args.formats]
        if is_app and lunahook_dir is not None:
            inner += ['--lunahook', '/lunahook']
        elif is_app and args.without_lunahook:
            inner += ['--without-lunahook']
        if is_wine:
            inner += ['--work-dir', '/cache/work/' + recipe_key(repo)]
        container_name = 'whisky-build-' + uuid.uuid4().hex[:12]
        run = ['docker', 'run', '--rm', '--init', '--name', container_name, '--platform', docker_platform,
               '--user', f'{os.getuid()}:{os.getgid()}',
               '--mount', mount(ROOT, '/workspace', readonly=True),
               '--mount', mount(output, '/output'),
               '--mount', mount(cache, '/cache'),
               '--env', 'HOME=/cache/home', '--env', 'CARGO_HOME=/cache/cargo',
               '--env', 'RUSTUP_HOME=/opt/rustup',
               '--env', 'WHISKY_LINUX_BUILD_CACHE=/cache/compile',
               '--env', 'WHISKY_PACKAGING_TOOL_CACHE=/cache/packaging-tools',
               '--env', 'GIT_CONFIG_COUNT=2', '--env', 'GIT_CONFIG_KEY_0=safe.directory',
               '--env', 'GIT_CONFIG_VALUE_0=/workspace', '--env', 'GIT_CONFIG_KEY_1=safe.directory',
               '--env', 'GIT_CONFIG_VALUE_1=/wine']
        if is_wine:
            run += ['--mount', mount(repo, '/wine', readonly=True)]
        if is_app and lunahook_dir is not None:
            run += ['--mount', mount(lunahook_dir, '/lunahook', readonly=True)]
        run += [image] + inner
        context = output / 'docker-context'
        result.update(cacheDirectory=str(cache), containerName=container_name,
            workDirectory='/cache/work/' + recipe_key(repo) if is_wine else None,
            contextFiles={str(dockerfile): 'Dockerfile', str(dependency_script): 'install-deps.sh'},
            environment={}, steps=[
            dict(name='缓存构建环境镜像', cwd=str(ROOT), command=['docker', 'build', '--platform', docker_platform,
                 '--file', str(context / 'Dockerfile'), '--tag', image, str(context)]),
            dict(name='容器内构建和验证', cwd=str(ROOT), command=run)])
    return result


def doctor(args):
    system, machine, _, tools = TARGETS[args.target]
    current_machine = {'aarch64': 'arm64', 'AMD64': 'x86_64'}.get(platform.machine(), platform.machine())
    expected_machine = {'x86_64': 'x86_64', 'arm64': 'arm64'}[machine]
    checks = []
    if args.executor == 'docker':
        tools = ('docker',)
        checks.append(dict(name='Linux Docker 目标', passed=system == 'Linux'))
    else:
        checks.append(dict(name=f'主机 {system}/{machine}',
                           passed=platform.system() == system and current_machine == expected_machine))
    for name in ('python3', 'git') + tools:
        checks.append(dict(name=name, passed=bool(shutil.which(name))))
    if args.executor == 'native' and system == 'Linux':
        release = Path('/etc/os-release').read_text() if Path('/etc/os-release').exists() else ''
        checks.append(dict(name='Ubuntu 24.04 构建基线', passed=bool(re.search(r'^ID=ubuntu$', release, re.M))
                           and bool(re.search(r'^VERSION_ID="?24\.04"?$', release, re.M))))
        checks.append(dict(name='Python 3.12+', passed=sys.version_info >= (3, 12)))
        if args.target in LINUX_APP_TARGETS:
            formats = args.formats or ['tar', 'deb', 'rpm', 'appimage']
            for name, tool in [('deb', 'dpkg-deb'), ('rpm', 'rpmbuild'), ('appimage', 'file')]:
                if name in formats:
                    checks.append(dict(name=tool, passed=bool(shutil.which(tool))))
        if args.target == 'wine-linux-amd64' and not args.compile_only:
            checks.append(dict(name='xvfb-run', passed=bool(shutil.which('xvfb-run'))))
    if args.target in APP_TARGETS:
        if args.without_lunahook:
            checks.append(dict(name='LunaHook 捕获载荷（已通过 --without-lunahook 显式跳过）', passed=True))
        else:
            raw = args.lunahook or (Path(os.environ['MALTORIA_LUNAHOOK_ARTIFACTS'])
                                    if os.environ.get('MALTORIA_LUNAHOOK_ARTIFACTS', '').strip() else None)
            if raw is None:
                if args.executor == 'native':
                    checks.append(lunahook_toolchain_check())
                else:
                    checks.append(dict(
                        name='LunaHook 捕获载荷（Docker 构建需 --lunahook PATH、MALTORIA_LUNAHOOK_ARTIFACTS 或 --without-lunahook；镜像不含交叉编译工具链）',
                        passed=False,
                    ))
            else:
                try:
                    resolved = resolve_lunahook_dir(raw)
                    checks.append(dict(name=f'LunaHook 捕获载荷（{resolved}）', passed=True))
                except ValueError as err:
                    checks.append(dict(name=f'LunaHook 捕获载荷（{err}）', passed=False))
    if args.target.startswith('wine-'):
        checks.append(dict(name='Wine 仓库配方', passed=(args.wine_repo / 'configs/crossover-source-lock.json').is_file()))
    if args.target == 'wine-macos-arm64' and platform.system() == 'Darwin':
        checks.append(dict(name='Wine 构建主机 macOS 26+', passed=int(platform.mac_ver()[0].split('.')[0]) >= 26))
    if args.executor == 'docker' and shutil.which('docker'):
        try:
            check = subprocess.run(['docker', 'info', '--format', '{{.OSType}}'], capture_output=True, text=True, timeout=10)
            ready = check.returncode == 0 and check.stdout.strip() == 'linux'
        except (OSError, subprocess.SubprocessError):
            ready = False
        checks.append(dict(name='Docker Linux 服务', passed=ready))
    return dict(target=args.target, passed=all(c['passed'] for c in checks), checks=checks,
                scope='主机、基础工具与配方；编译过程继续校验库、源码及证书')


def lunahook_toolchain_check(builder=None):
    # 与 scripts/build-lunahook.py 共用工具查找与 xwin 缓存判断；SDK/CRT 缓存已覆盖
    # x86+x64 时无需 xwin 下载器。
    builder = builder or lunahook_builder()
    missing = [tool for tool in LUNAHOOK_TOOLS if builder.locate_tool(tool) is None]
    xwin_dir = builder.default_xwin_dir().expanduser()
    if builder.xwin_sysroot_ready(xwin_dir, LUNAHOOK_ARCHES):
        sysroot = f'SDK/CRT 缓存就绪：{xwin_dir}'
    else:
        sysroot = f'SDK/CRT 缓存缺 x86/x64：{xwin_dir}，需 xwin 下载'
        if builder.locate_tool('xwin') is None:
            missing.append('xwin')
    return dict(
        name='LunaHook 本机交叉编译工具链（缺：' + (', '.join(missing) if missing else '无') + '；' + sysroot
             + '；macOS: brew install llvm lld ninja xwin mono，'
               'Ubuntu/Debian: LLVM>=19, cmake>=3.25, ninja-build, mono-mcs, xwin）',
        passed=not missing)


def show(value, as_json=False):
    if as_json:
        print(json.dumps(value, ensure_ascii=False, indent=2))
        return
    print(value['target'], '·', value.get('description', '环境检查'))
    if 'checks' in value:
        for item in value['checks']:
            print('  ' + ('✓' if item['passed'] else '需准备') + ' ' + item['name'])
        return
    print('输出：', value['output'])
    if value.get('workDirectory'):
        print('Wine 工作目录：', value['workDirectory'])
    if value.get('target') in APP_TARGETS:
        if value.get('lunahookBuild'):
            print('LunaHook 载荷：本机交叉编译（构建步骤会自动产出并打包进应用）')
        elif value.get('lunahook'):
            print('LunaHook 载荷：', value['lunahook'])
        elif value.get('withoutLunahook'):
            print('LunaHook 载荷：已显式跳过（--without-lunahook，游戏内文本捕获将不可用）')
        else:
            print('⚠️  LunaHook 载荷：未指定（原生执行器下 run 会自动本机交叉编译；Docker 执行器需 --lunahook PATH、MALTORIA_LUNAHOOK_ARTIFACTS 或 --without-lunahook）')
    for index, step in enumerate(value['steps'], 1):
        print(f"  {index}. {step['name']}\n     {shlex.join(step['command'])}")
    print('环境：', json.dumps(value['environment'], ensure_ascii=False))


def execute(value):
    output = Path(value['output'])
    output.mkdir(parents=True, exist_ok=False)
    if value.get('cacheDirectory'):
        Path(value['cacheDirectory']).mkdir(parents=True, exist_ok=True)
        (Path(value['cacheDirectory']) / 'home').mkdir(exist_ok=True)
        context = output / 'docker-context'
        context.mkdir()
        for source, name in value['contextFiles'].items():
            shutil.copyfile(source, context / name)
    report = dict(value, status='running', startedAt=datetime.now(timezone.utc).isoformat(), results=[])
    report_path = output / 'build-report.json'

    def save():
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')

    save()
    started = time.monotonic()
    process = None
    lock = None
    try:
        if value['target'].startswith('wine-') and value['executor'] == 'native':
            work = Path(value['workDirectory'])
            work.parent.mkdir(parents=True, exist_ok=True)
            lock = (work.parent / (work.name + '.build.lock')).open('a')
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise ValueError('Wine 工作目录正在构建：' + str(work)) from error
        for index, step in enumerate(value['steps'], 1):
            log = output / f'{index:02d}.log'
            print(f"[{index}/{len(value['steps'])}] {step['name']} · {log}", flush=True)
            before = time.monotonic()
            with log.open('w') as stream:
                process = subprocess.Popen(step['command'], cwd=step['cwd'],
                    env=dict(os.environ, **value['environment']), stdout=stream, stderr=subprocess.STDOUT,
                    start_new_session=True)
                while process.poll() is None:
                    try:
                        process.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        print(f"  构建进行中 · {int(time.monotonic() - before)} 秒 · {log}", flush=True)
                result = process.returncode
            report['results'].append(dict(name=step['name'], exitCode=result, log=str(log),
                                          seconds=round(time.monotonic() - before, 2)))
            save()
            if result:
                print(log.read_text(errors='replace')[-12000:], file=sys.stderr)
                raise subprocess.CalledProcessError(result, step['command'])
        report['status'] = 'passed'
    except BaseException as error:
        report['status'] = 'cancelled' if isinstance(error, KeyboardInterrupt) else 'failed'
        report['error'] = str(error)
        if process and process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
        if value.get('containerName'):
            try:
                subprocess.run(['docker', 'stop', '--time', '5', value['containerName']],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
            except (OSError, subprocess.SubprocessError):
                pass
        raise
    finally:
        if lock:
            lock.close()
        report['seconds'] = round(time.monotonic() - started, 2)
        save()
        print('构建记录：', report_path, flush=True)


def choose_lunahook(args):
    detected = discover_local_lunahook()
    if args.executor == 'docker':
        # Docker 镜像不含交叉编译工具链：不提供本机编译，检测到载荷时默认复用，
        # 否则必须输入路径或显式 skip。
        if detected:
            options = {'1': 'reuse', '2': 'path'}
            prompt = f'LunaHook 载荷（1. 复用 {detected}  2. 输入其他路径  skip. 跳过）[1]：'
            default = '1'
        else:
            options = {}
            prompt = 'LunaHook 载荷目录（Docker 构建必填，或输入 skip 跳过）：'
            default = None
    else:
        if detected:
            options = {'1': 'build', '2': 'reuse', '3': 'path'}
            prompt = f'LunaHook 载荷（1. 本机交叉编译  2. 复用 {detected}  3. 输入其他路径  skip. 跳过）[1]：'
        else:
            options = {'1': 'build', '2': 'path'}
            prompt = 'LunaHook 载荷（1. 本机交叉编译  2. 输入路径  skip. 跳过）[1]：'
        default = '1'
    entered = input(prompt).strip() or default
    if not entered:
        raise ValueError('Docker 构建需要 LunaHook 载荷目录，或输入 skip 显式跳过')
    choice = 'skip' if entered.lower() == 'skip' else options.get(entered)
    if choice == 'skip':
        args.without_lunahook = True
        print('提示：已跳过 LunaHook，打包后的应用无法使用游戏内文本捕获。')
    elif choice == 'build':
        print('提示：将使用 clang-cl/xwin 在本机交叉编译 LunaHook（无需 Windows）。')
    elif choice == 'reuse':
        args.lunahook = detected
    elif choice == 'path':
        args.lunahook = resolve_lunahook_dir(input('LunaHook 载荷目录：').strip())
    else:
        args.lunahook = resolve_lunahook_dir(entered)


def menu(args):
    if not sys.stdin.isatty():
        raise ValueError('交互菜单需要终端；CI 使用 list、doctor、plan 或 run')
    print('Maltoria / Wine 构建中心\n')
    names = list(TARGETS)
    for index, name in enumerate(names, 1):
        print(f'  {index}. {name} · {TARGETS[name][2]}')
    print('  0. 退出')
    selected = input('\n选择目标：').strip()
    if selected == '0':
        return
    if selected not in [str(n) for n in range(1, len(names) + 1)]:
        raise ValueError('请输入菜单中的编号')
    args.target = names[int(selected) - 1]
    if args.target.startswith('wine-'):
        args.wine_repo = Path(input(f'Wine 仓库 [{args.wine_repo}]：').strip() or args.wine_repo).expanduser().resolve()
    if TARGETS[args.target][0] == 'Linux':
        executor = input('执行环境：1. Ubuntu 24.04 主机  2. Docker [1]：').strip() or '1'
        if executor not in ('1', '2'):
            raise ValueError('请选择 1 或 2')
        args.executor = 'docker' if executor == '2' else 'native'
    if args.target in LINUX_APP_TARGETS:
        args.version = args.version or input('安装包版本 X.Y.Z [0.1.0]：').strip() or '0.1.0'
        format_choice = input('格式：1. 全部  2. tar  3. deb  4. rpm  5. AppImage [1]：').strip() or '1'
        choices = {'1': ['tar', 'deb', 'rpm', 'appimage'], '2': ['tar'], '3': ['deb'],
                   '4': ['rpm'], '5': ['appimage']}
        if format_choice not in choices:
            raise ValueError('请选择 1 至 5')
        args.formats = choices[format_choice]
    if args.target in APP_TARGETS and not args.lunahook and not args.without_lunahook:
        choose_lunahook(args)
    if args.target == 'wine-linux-amd64':
        mode = input('验证：1. 运行回归  2. compile-only [1]：').strip() or '1'
        if mode not in ('1', '2'):
            raise ValueError('请选择 1 或 2')
        args.compile_only = mode == '2'
    value = plan(args)
    show(value)
    actions = '\n1. 环境检查  2. 开始构建'
    if args.target == 'whisky-macos-gpui-arm64':
        actions += '  3. 构建并部署到 /Applications'
    print(actions + '  0. 返回终端')
    action = input('选择 [1]：').strip() or '1'
    if action == '3' and args.target == 'whisky-macos-gpui-arm64':
        args.install = True
        value = plan(args)
        show(value)
    if action == '0':
        return
    if action not in ('1', '2') and not (action == '3' and args.install):
        raise ValueError('请选择菜单中的操作')
    checks = doctor(args)
    show(checks)
    if action in ('2', '3'):
        if not checks['passed']:
            raise ValueError('请准备环境检查列出的工具与主机')
        execute(value)


def main(argv=None):
    args = parser().parse_args(argv)
    args.wine_repo = args.wine_repo.expanduser().resolve()
    def interrupted(_signum, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupted)
    try:
        if args.action == 'list':
            values = [dict(target=k, host=v[0], architecture=v[1], description=v[2]) for k, v in TARGETS.items()]
            print(json.dumps(values, ensure_ascii=False, indent=2) if args.json else
                  '\n'.join(f"{v['target']:28} {v['description']}" for v in values))
        elif args.action == 'menu':
            menu(args)
        elif not args.target:
            raise ValueError('请提供构建目标；使用 ./build.sh list 查看')
        elif args.action == 'doctor':
            value = doctor(args)
            show(value, args.json)
            return 0 if value['passed'] else 1
        else:
            value = plan(args)
            if args.action == 'run' and args.target in APP_TARGETS and not value.get('lunahook') and not args.without_lunahook:
                raise ValueError(missing_lunahook_message(args.target))
            show(value, args.json)
            if args.action == 'run':
                checks = doctor(args)
                if not checks['passed']:
                    show(checks)
                    raise ValueError('请准备环境检查列出的工具与主机')
                execute(value)
        return 0
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print('构建错误：' + str(error), file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print('\n构建已取消，日志与已生成文件保留。', file=sys.stderr)
        return 130


if __name__ == '__main__':
    sys.exit(main())
