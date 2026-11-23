#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Cross-compile LunaHook and MaltoriaLunaHost on macOS/Linux using clang-cl + lld-link + xwin."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import uuid

ROOT = Path(__file__).resolve().parent.parent

LLVM_SEARCH_DIRS = (
    Path("/opt/homebrew/opt/llvm/bin"),
    Path("/opt/homebrew/opt/llvm@22/bin"),
    Path("/opt/homebrew/opt/lld/bin"),
    Path("/usr/local/opt/llvm/bin"),
    Path("/usr/local/opt/lld/bin"),
)


def find_tool(name, extra_dirs=()):
    candidates = [Path(d) for d in extra_dirs if d]
    llvm_env = os.environ.get("LLVM_DIR")
    if llvm_env:
        candidates.insert(0, Path(llvm_env) / "bin")
        candidates.insert(0, Path(llvm_env))
    candidates.extend(LLVM_SEARCH_DIRS)

    found = shutil.which(name)
    if found:
        return Path(found)
    for directory in candidates:
        candidate = directory / name
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
    raise FileNotFoundError(
        f"Required tool '{name}' not found in PATH or standard LLVM locations. "
        f"On macOS: brew install llvm lld ninja xwin mono; "
        f"on Ubuntu/Debian: apt-get install clang lld llvm ninja-build mono-mcs"
    )


def ensure_xwin(xwin_dir, arches, accept_license):
    xwin_dir = xwin_dir.resolve()
    needed_lib_arches = [("x86" if a == "x86" else "x86_64") for a in arches]

    def is_ready():
        if not ((xwin_dir / "crt/include").is_dir() and (xwin_dir / "sdk/include").is_dir()):
            return False
        for lib_arch in needed_lib_arches:
            if not (xwin_dir / f"crt/lib/{lib_arch}").is_dir():
                return False
            if not (xwin_dir / f"sdk/lib/um/{lib_arch}").is_dir():
                return False
            if not (xwin_dir / f"sdk/lib/ucrt/{lib_arch}").is_dir():
                return False
        return True

    if is_ready():
        return xwin_dir

    if not accept_license:
        raise RuntimeError(
            f"xwin sysroot at {xwin_dir} is missing required architectures ({', '.join(needed_lib_arches)}). "
            f"Run with --accept-xwin-license to download the Microsoft CRT & Windows SDK via xwin, "
            f"or pre-populate it with: xwin --accept-license --arch x86,x86_64 splat --output {xwin_dir}"
        )

    xwin_bin = find_tool("xwin")
    xwin_dir.parent.mkdir(parents=True, exist_ok=True)
    xwin_cache = xwin_dir.parent / f"{xwin_dir.name}.download-cache"
    cmd = [
        str(xwin_bin),
        "--accept-license",
        "--cache-dir",
        str(xwin_cache),
        "--arch",
        "x86,x86_64",
        "splat",
        "--output",
        str(xwin_dir),
    ]
    print(f"Downloading Windows SDK & CRT (x86, x86_64) via xwin into {xwin_dir} ...")
    subprocess.run(cmd, check=True)
    if not is_ready():
        raise RuntimeError(f"xwin splat did not produce expected crt/ and sdk/ directories in {xwin_dir}")
    return xwin_dir


def write_toolchain_file(path, arch, xwin_dir, tools):
    if arch == "x86":
        triple = "i686-pc-windows-msvc"
        processor = "X86"
        lib_arch = "x86"
        machine = "X86"
    elif arch == "x64":
        triple = "x86_64-pc-windows-msvc"
        processor = "AMD64"
        lib_arch = "x86_64"
        machine = "X64"
    else:
        raise ValueError(f"Unsupported architecture: {arch}")

    crt_inc = (xwin_dir / "crt/include").as_posix()
    ucrt_inc = (xwin_dir / "sdk/include/ucrt").as_posix()
    um_inc = (xwin_dir / "sdk/include/um").as_posix()
    shared_inc = (xwin_dir / "sdk/include/shared").as_posix()

    crt_lib = (xwin_dir / f"crt/lib/{lib_arch}").as_posix()
    ucrt_lib = (xwin_dir / f"sdk/lib/ucrt/{lib_arch}").as_posix()
    um_lib = (xwin_dir / f"sdk/lib/um/{lib_arch}").as_posix()

    common_compile_flags = (
        f"-fms-compatibility -fms-extensions "
        f"-Wno-unused-command-line-argument -Wno-clang-cl-pch -Wno-c++11-narrowing "
        f"-imsvc \\\"{crt_inc}\\\" "
        f"-imsvc \\\"{ucrt_inc}\\\" "
        f"-imsvc \\\"{um_inc}\\\" "
        f"-imsvc \\\"{shared_inc}\\\""
    )
    common_link_flags = (
        f"/machine:{machine} "
        f"/libpath:\\\"{crt_lib}\\\" "
        f"/libpath:\\\"{ucrt_lib}\\\" "
        f"/libpath:\\\"{um_lib}\\\""
    )
    rc_flags = f"/I \\\"{crt_inc}\\\" /I \\\"{ucrt_inc}\\\" /I \\\"{um_inc}\\\" /I \\\"{shared_inc}\\\""

    content = f"""# Auto-generated CMake toolchain file for clang-cl + lld-link + xwin ({arch})
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR {processor})
set(CMAKE_SYSTEM_VERSION 10.0)

set(CMAKE_C_COMPILER "{tools['clang-cl'].as_posix()}")
set(CMAKE_CXX_COMPILER "{tools['clang-cl'].as_posix()}")
set(CMAKE_LINKER "{tools['lld-link'].as_posix()}")
set(CMAKE_AR "{tools['llvm-lib'].as_posix()}")
set(CMAKE_RC_COMPILER "{tools['llvm-rc'].as_posix()}")
set(CMAKE_MT "{tools['llvm-mt'].as_posix()}")

set(CMAKE_C_COMPILER_TARGET {triple})
set(CMAKE_CXX_COMPILER_TARGET {triple})

set(CMAKE_TRY_COMPILE_CONFIGURATION Release)
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")

set(CMAKE_C_FLAGS_INIT "{common_compile_flags}")
set(CMAKE_CXX_FLAGS_INIT "{common_compile_flags}")

set(CMAKE_EXE_LINKER_FLAGS_INIT "{common_link_flags}")
set(CMAKE_SHARED_LINKER_FLAGS_INIT "{common_link_flags}")
set(CMAKE_MODULE_LINKER_FLAGS_INIT "{common_link_flags}")
set(CMAKE_RC_FLAGS_INIT "{rc_flags}")

# Skip try_compile linking issues with cross-compilation when needed; CMake still checks compiler ID.
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
"""
    path.write_text(content, encoding="utf-8")


def run_checked(cmd, cwd=None):
    print("+", " ".join(str(part) for part in cmd), flush=True)
    subprocess.run([str(part) for part in cmd], cwd=cwd, check=True)


def build_arch(arch, output_root, xwin_dir, tools, jobs, keep_temp=False):
    bits = "32" if arch == "x86" else "64"
    temp_base = Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir())
    work = temp_base / f"MaltoriaNativeCross-{arch}-{uuid.uuid4().hex}"
    work.mkdir(parents=True, exist_ok=False)

    try:
        native = work / "sources"
        lunahook_build = work / "lunahook"
        host_build = work / "host"
        toolchain = work / f"toolchain-{arch}.cmake"

        write_toolchain_file(toolchain, arch, xwin_dir, tools)

        run_checked([
            sys.executable,
            ROOT / "scripts/prepare-native-sources.py",
            "--output",
            native,
        ])

        source = native / "LunaTranslator/src/NativeImpl/LunaHook"
        csc_cmd = f"{tools['mcs'].as_posix()};-nologo;-target:library"

        configure_lunahook = [
            tools["cmake"],
            "-G", "Ninja",
            f"-DCMAKE_MAKE_PROGRAM={tools['ninja'].as_posix()}",
            f"-DCMAKE_TOOLCHAIN_FILE={toolchain.as_posix()}",
            "-DCMAKE_BUILD_TYPE=Release",
            "-S", source,
            "-B", lunahook_build,
            "-DBUILD_HOST=ON",
            "-DBUILD_HOOK=ON",
            "-DBUILD_GUI=OFF",
            "-DWIN10ABOVE=ON",
            "-DUSE_VC_LTL=OFF",
            f"-DLUNA_CSC_COMMAND={csc_cmd}",
            f"-DFETCHCONTENT_SOURCE_DIR_MINHOOK={(native / 'MinHook').as_posix()}",
            f"-DFETCHCONTENT_SOURCE_DIR_UCHARDET={(native / 'uchardet').as_posix()}",
            "-DFETCHCONTENT_FULLY_DISCONNECTED=ON",
        ]
        run_checked(configure_lunahook)

        build_cmd = [
            tools["cmake"],
            "--build", lunahook_build,
            "--target", "LunaHostDll", "LunaHook", "LunaTmpFontLoader",
        ]
        if jobs:
            build_cmd.extend(["--parallel", str(jobs)])
        else:
            build_cmd.append("--parallel")
        run_checked(build_cmd)

        nlohmann_include = native / "nlohmann-json/single_include"
        host_source = ROOT / "Native/MaltoriaLunaHost"
        configure_host = [
            tools["cmake"],
            "-G", "Ninja",
            f"-DCMAKE_MAKE_PROGRAM={tools['ninja'].as_posix()}",
            f"-DCMAKE_TOOLCHAIN_FILE={toolchain.as_posix()}",
            "-DCMAKE_BUILD_TYPE=Release",
            "-S", host_source,
            "-B", host_build,
            f"-DNLOHMANN_JSON_INCLUDE_DIR={nlohmann_include.as_posix()}",
        ]
        run_checked(configure_host)

        build_host_cmd = [
            tools["cmake"],
            "--build", host_build,
            "--target", "MaltoriaLunaHost", "MaltoriaUnicodeFixture",
        ]
        if jobs:
            build_host_cmd.extend(["--parallel", str(jobs)])
        else:
            build_host_cmd.append("--parallel")
        run_checked(build_host_cmd)

        output_root.mkdir(parents=True, exist_ok=True)
        payload = output_root / arch
        if payload.exists():
            shutil.rmtree(payload)
        payload.mkdir(parents=True, exist_ok=True)

        binary_dir = source / "builds/Release_win10"
        for name in (f"LunaHost{bits}.dll", f"LunaHook{bits}.dll", "LunaTmpFontLoader.dll"):
            src_file = binary_dir / name
            if not src_file.is_file():
                raise FileNotFoundError(f"Missing expected build output: {src_file}")
            shutil.copy2(src_file, payload / name)

        for name in (f"MaltoriaLunaHost{bits}.exe", "MaltoriaUnicodeFixture.exe"):
            src_file = host_build / name
            if not src_file.is_file():
                src_file = host_build / "Release" / name
            if not src_file.is_file():
                raise FileNotFoundError(f"Missing expected build output: {name}")
            shutil.copy2(src_file, payload / name)

        print(f"Staged fresh {arch} cross-compiled native build under {payload}")
    finally:
        if not keep_temp:
            shutil.rmtree(work, ignore_errors=True)
        else:
            print(f"Kept temporary build directory: {work}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--architecture", "-a",
        choices=("x86", "x64", "all"),
        required=True,
        help="Target Windows architecture to build",
    )
    parser.add_argument(
        "--output", "-o",
        type=Path,
        required=True,
        help="Output directory outside the repository checkout",
    )
    parser.add_argument(
        "--xwin-dir",
        type=Path,
        default=Path(os.environ.get("XWIN_DIR", Path.home() / ".cache/maltoria-xwin")),
        help="Directory containing xwin-splatted Windows SDK & CRT",
    )
    parser.add_argument(
        "--accept-xwin-license",
        action="store_true",
        help="Automatically run 'xwin --accept-license splat' if --xwin-dir is missing",
    )
    parser.add_argument(
        "--jobs", "-j",
        type=int,
        default=None,
        help="Parallel build jobs",
    )
    parser.add_argument(
        "--keep-temp",
        action="store_true",
        help="Keep temporary build directory for debugging",
    )
    args = parser.parse_args(argv)

    output_root = args.output.resolve()
    try:
        output_root.relative_to(ROOT)
        raise ValueError("Output must be outside the source checkout.")
    except ValueError as exc:
        if "Output must be outside" in str(exc):
            raise

    tools = {
        name: find_tool(name)
        for name in ("cmake", "ninja", "clang-cl", "lld-link", "llvm-lib", "llvm-rc", "llvm-mt", "mcs")
    }
    arches = ("x86", "x64") if args.architecture == "all" else (args.architecture,)
    xwin_dir = ensure_xwin(args.xwin_dir, arches, args.accept_xwin_license)

    for arch in arches:
        build_arch(arch, output_root, xwin_dir, tools, args.jobs, keep_temp=args.keep_temp)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (FileNotFoundError, RuntimeError, subprocess.CalledProcessError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
