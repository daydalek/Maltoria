#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Verify and safely extract locked LunaHook build sources."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "ThirdParty/manifest.json"
PATCH = ROOT / "ThirdParty/lunahook-minhook-pin.patch"
MINHOOK_REVISION = "c3fcafdc10146beb5919319d0683e44e3c30d537"


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def safe_extract(archive_path, destination, revision):
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        if not members or len(members) > 100_000:
            raise ValueError(f"Unexpected archive member count: {archive_path.name}")
        roots = set()
        entries = {}
        planned = []
        total_size = 0
        for member in members:
            name = member.name
            if not name or "\0" in name or "\\" in name or name.startswith("/"):
                raise ValueError(f"Unsafe archive path: {member.name}")
            if member.issym() or member.islnk() or not (member.isfile() or member.isdir()):
                raise ValueError(f"Unsupported archive member: {member.name}")
            parts = name.split("/")
            if member.isdir() and parts[-1] == "":
                parts.pop()
            if (not parts or any(part in ("", ".", "..") or ":" in part
                                 for part in parts)):
                raise ValueError(f"Unsafe archive path: {member.name}")
            path = PurePosixPath(name)
            if path.is_absolute() or not path.parts:
                raise ValueError(f"Unsafe archive path: {member.name}")
            roots.add(parts[0])
            if len(parts) == 1:
                if not member.isdir():
                    raise ValueError(f"Archive root is not a directory: {member.name}")
                continue
            relative = tuple(parts[1:])
            if relative in entries:
                raise ValueError(f"Duplicate archive path: {member.name}")
            if member.isfile() and (member.size < 0 or member.size > 100 * 1024 * 1024):
                raise ValueError(f"Oversized archive member: {member.name}")
            total_size += member.size if member.isfile() else 0
            if total_size > 2 * 1024 * 1024 * 1024:
                raise ValueError(f"Oversized archive contents: {archive_path.name}")
            entries[relative] = member.isdir()
            planned.append((member, relative))
        if len(roots) != 1 or not next(iter(roots)).endswith(revision):
            raise ValueError(f"Unexpected archive root for {archive_path.name}: {sorted(roots)}")
        for relative, is_directory in entries.items():
            for count in range(1, len(relative)):
                parent = relative[:count]
                if parent in entries and not entries[parent]:
                    raise ValueError(f"Archive path traverses a file: {'/'.join(relative)}")

        if destination.exists():
            raise ValueError(f"Extraction destination already exists: {destination}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        staging = Path(tempfile.mkdtemp(prefix=f"{destination.name}.extract-",
                                        dir=destination.parent))
        try:
            for member, relative in planned:
                target = staging.joinpath(*relative)
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                source = archive.extractfile(member)
                if source is None:
                    raise ValueError(f"Missing archive data: {member.name}")
                written = 0
                with source, target.open("xb") as output:
                    while chunk := source.read(1024 * 1024):
                        written += len(chunk)
                        if written > member.size:
                            raise ValueError(f"Unexpected archive data length: {member.name}")
                        output.write(chunk)
                if written != member.size:
                    raise ValueError(f"Truncated archive member: {member.name}")
            staging.replace(destination)
        except BaseException:
            shutil.rmtree(staging, ignore_errors=True)
            raise


def prepare(output):
    manifest = json.loads(MANIFEST.read_text())
    packages = {package["name"]: package for package in manifest["native_packages"]}
    if output.exists() and any(output.iterdir()):
        raise ValueError(f"Output directory must be empty: {output}")
    output.mkdir(parents=True, exist_ok=True)
    for name in packages:
        package = packages[name]
        archive = ROOT / "ThirdParty" / package["archive"]
        if not archive.is_file() or digest(archive) != package["sha256"]:
            raise ValueError(f"Source archive hash mismatch: {package['archive']}")
        if package["sha256"] != manifest["files"].get(package["archive"]):
            raise ValueError(f"Source archive inventory mismatch: {package['archive']}")
        safe_extract(archive, output / package["directory"], package["revision"])

    luna = output / packages["lunatranslator"]["directory"]
    cmake = luna / "src/NativeImpl/LunaHook/LunaHook/CMakeLists.txt"
    text = cmake.read_text(encoding="utf-8")
    patch = PATCH.read_text(encoding="utf-8")
    old = "\tGIT_TAG master\n\tGIT_SHALLOW TRUE"
    new = f"\tGIT_TAG {MINHOOK_REVISION}\n\tGIT_SHALLOW FALSE"
    if (text.count(old) != 1 or patch.count("-\tGIT_TAG master\n-\tGIT_SHALLOW TRUE") != 1
            or f"+\tGIT_TAG {MINHOOK_REVISION}\n+\tGIT_SHALLOW FALSE" not in patch):
        raise ValueError("Pinned MinHook patch no longer matches the locked LunaTranslator source")
    patched = text.replace(old, new)
    cmake.write_text(patched, encoding="utf-8")
    final_text = cmake.read_text(encoding="utf-8")
    if MINHOOK_REVISION not in final_text or "GIT_TAG master" in final_text:
        raise ValueError("MinHook pin patch verification failed")
    main = luna / "src/NativeImpl/LunaHook/LunaHook/main.cc"
    text = main.read_text(encoding="utf-8")
    signature = "BOOL WINAPI DllMain(HINSTANCE hModule, DWORD fdwReason, LPVOID)"
    detach = "\tcase DLL_PROCESS_DETACH:\n\t{\n\t\tdetachall();"
    replacement = ("\tcase DLL_PROCESS_DETACH:\n\t{\n"
                   "\t\t// Maltoria: process teardown has already terminated other threads.\n"
                   "\t\t// They may own locks used by hook cleanup. The OS reclaims memory;\n"
                   "\t\t// only an explicit FreeLibrary should run synchronized cleanup.\n"
                   "\t\tif (reserved) return TRUE;\n\t\tdetachall();")
    if text.count(signature) != 1 or text.count(detach) != 1:
        raise ValueError("Process-exit patch no longer matches the locked LunaTranslator source")
    text = text.replace(signature, signature[:-1] + " reserved)").replace(detach, replacement)
    main.write_text(text, encoding="utf-8")

    # Maltoria: cross-compile compatibility (ATL-free CComPtr fallback + configurable C# compiler).
    python_cpp = luna / "src/NativeImpl/LunaHook/LunaHook/engines/python/python.cpp"
    py_text = python_cpp.read_text(encoding="utf-8")
    atl_old = "#include <atlbase.h>"
    atl_new = (
        "#if defined(__has_include) && __has_include(<atlbase.h>)\n"
        "#include <atlbase.h>\n"
        "#else\n"
        "// Maltoria: minimal CComPtr fallback so cross-compilation with xwin does not require ATL.\n"
        "template <class T>\n"
        "struct CComPtr\n"
        "{\n"
        "    T *p = nullptr;\n"
        "    CComPtr() = default;\n"
        "    ~CComPtr()\n"
        "    {\n"
        "        if (p)\n"
        "            p->Release();\n"
        "    }\n"
        "    CComPtr(const CComPtr &) = delete;\n"
        "    CComPtr &operator=(const CComPtr &) = delete;\n"
        "    T **operator&() { return &p; }\n"
        "    T *operator->() const { return p; }\n"
        "    void Attach(T *ptr)\n"
        "    {\n"
        "        if (p)\n"
        "            p->Release();\n"
        "        p = ptr;\n"
        "    }\n"
        "    template <class Q>\n"
        "    HRESULT QueryInterface(Q **pp) const\n"
        "    {\n"
        "        return p ? p->QueryInterface(__uuidof(Q), reinterpret_cast<void **>(pp)) : E_POINTER;\n"
        "    }\n"
        "};\n"
        "#endif"
    )
    if py_text.count(atl_old) != 1:
        raise ValueError("Cross-compile ATL patch no longer matches the locked LunaTranslator source")
    python_cpp.write_text(py_text.replace(atl_old, atl_new), encoding="utf-8")

    engines_cmake = luna / "src/NativeImpl/LunaHook/LunaHook/engines/CMakeLists.txt"
    ec_text = engines_cmake.read_text(encoding="utf-8")
    csc_old = (
        'add_custom_command(\n'
        '    OUTPUT "${_lunatmp_dll}"\n'
        '    COMMAND ${CMAKE_COMMAND} -E make_directory "${binary_out_putpath}"\n'
        '    COMMAND "${csc}" /nologo /nostdlib /target:library /out:${_lunatmp_dll} /reference:${mscorlib} LunaTmpFontLoader.cs\n'
        '    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/unity"\n'
        '    DEPENDS "${_lunatmp_src}"\n'
        '    VERBATIM\n'
        ')'
    )
    csc_new = (
        'if(DEFINED LUNA_CSC_COMMAND)\n'
        'add_custom_command(\n'
        '    OUTPUT "${_lunatmp_dll}"\n'
        '    COMMAND ${CMAKE_COMMAND} -E make_directory "${binary_out_putpath}"\n'
        '    COMMAND ${LUNA_CSC_COMMAND} -out:${_lunatmp_dll} LunaTmpFontLoader.cs\n'
        '    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/unity"\n'
        '    DEPENDS "${_lunatmp_src}"\n'
        '    VERBATIM\n'
        ')\n'
        'else()\n'
        + csc_old + '\n'
        'endif()'
    )
    if ec_text.count(csc_old) != 1:
        raise ValueError("Cross-compile C# patch no longer matches the locked LunaTranslator source")
    engines_cmake.write_text(ec_text.replace(csc_old, csc_new), encoding="utf-8")

    # Avoid POSIX shell brace expansion of -DLUNA_VERSION_={a,b,c,d} under Ninja on macOS/Linux.
    version_cmake = luna / "src/NativeImpl/version.cmake"
    vc_text = version_cmake.read_text(encoding="utf-8")
    vc_old = "target_compile_definitions(VERSION_DEF PRIVATE\n    LUNA_VERSION_=${LUNA_VERSION})"
    vc_new = (
        "target_compile_definitions(VERSION_DEF PRIVATE\n"
        "    LUNA_VERSION_0=${VERSION_MAJOR}\n"
        "    LUNA_VERSION_1=${VERSION_MINOR}\n"
        "    LUNA_VERSION_2=${VERSION_PATCH}\n"
        "    LUNA_VERSION_3=${VERSION_REVISION})"
    )
    if vc_text.count(vc_old) != 1:
        raise ValueError("Cross-compile version.cmake patch no longer matches the locked LunaTranslator source")
    version_cmake.write_text(vc_text.replace(vc_old, vc_new), encoding="utf-8")

    version_def = luna / "src/NativeImpl/version_def.cpp"
    vd_text = version_def.read_text(encoding="utf-8")
    vd_old = "WORD LUNA_VERSION[4] = LUNA_VERSION_;"
    vd_new = "WORD LUNA_VERSION[4] = {LUNA_VERSION_0, LUNA_VERSION_1, LUNA_VERSION_2, LUNA_VERSION_3};"
    if vd_text.count(vd_old) != 1:
        raise ValueError("Cross-compile version_def.cpp patch no longer matches the locked LunaTranslator source")
    version_def.write_text(vd_text.replace(vd_old, vd_new), encoding="utf-8")

    # Standard C++17 rvalue overloads for helper functions called with temporaries under MSVC /permissive.
    stringutils = luna / "src/NativeImpl/LunaHook/include/stringutils.h"
    su_text = stringutils.read_text(encoding="utf-8")
    sr_old = 'std::u32string &strReplace(std::u32string &str, const std::u32string &oldStr, const std::u32string &newStr = U"");'
    sr_new = (
        sr_old + "\n"
        'inline std::string strReplace(std::string &&str, const std::string &oldStr, const std::string &newStr = "") { return std::move(strReplace(str, oldStr, newStr)); }\n'
        'inline std::wstring strReplace(std::wstring &&str, const std::wstring &oldStr, const std::wstring &newStr = L"") { return std::move(strReplace(str, oldStr, newStr)); }\n'
        'inline std::u32string strReplace(std::u32string &&str, const std::u32string &oldStr, const std::u32string &newStr = U"") { return std::move(strReplace(str, oldStr, newStr)); }'
    )
    trim_old = "template <class ST>\ninline ST &Trim(ST &text)\n{\n  text.erase(text.begin(), std::find_if_not(text.begin(), text.end(), iswspace));\n  text.erase(std::find_if_not(text.rbegin(), text.rend(), iswspace).base(), text.end());\n  return text;\n}"
    trim_new = (
        trim_old + "\n"
        "template <class ST, std::enable_if_t<!std::is_lvalue_reference_v<ST>, int> = 0>\n"
        "inline ST Trim(ST &&text) { return std::move(Trim(text)); }"
    )
    if su_text.count(sr_old) != 1 or su_text.count(trim_old) != 1:
        raise ValueError("Cross-compile stringutils.h patch no longer matches the locked LunaTranslator source")
    stringutils.write_text(su_text.replace(sr_old, sr_new).replace(trim_old, trim_new), encoding="utf-8")

    stringfilters = luna / "src/NativeImpl/LunaHook/LunaHook/util/stringfilters.h"
    sf_text = stringfilters.read_text(encoding="utf-8")
    rk_old = "std::wstring &remapkatakana(std::wstring &);"
    rk_new = (
        rk_old + "\n"
        "#ifndef MALTORIA_REMAPKATAKANA_RVAL_DEFINED\n"
        "#define MALTORIA_REMAPKATAKANA_RVAL_DEFINED\n"
        "inline std::wstring remapkatakana(std::wstring &&s) { return std::move(remapkatakana(s)); }\n"
        "#endif"
    )
    if sf_text.count(rk_old) != 1:
        raise ValueError("Cross-compile stringfilters.h patch no longer matches the locked LunaTranslator source")
    stringfilters.write_text(sf_text.replace(rk_old, rk_new), encoding="utf-8")

    # Fix MSVC-only rvalue-to-non-const-ref binding in KiriKiri.cpp and case-sensitive include paths for Linux.
    kirikiri = luna / "src/NativeImpl/LunaHook/LunaHook/engine32/KiriKiri.cpp"
    kk_text = kirikiri.read_text(encoding="utf-8")
    kk_old = "  auto filterkrkrz(std::wstring &ws)"
    kk_new = "  auto filterkrkrz(std::wstring ws)"
    if kk_text.count(kk_old) != 1:
        raise ValueError("Cross-compile KiriKiri.cpp patch no longer matches the locked LunaTranslator source")
    kirikiri.write_text(kk_text.replace(kk_old, kk_new), encoding="utf-8")

    for rel, old_inc, new_inc in (
        ("src/NativeImpl/LunaHook/LunaHook/engine32/ApricoT.cpp", '#include "ApRicoT.h"', '#include "ApricoT.h"'),
        ("src/NativeImpl/LunaHook/LunaHook/engine32/lucifen.cpp", '#include "Lucifen.h"', '#include "lucifen.h"'),
        ("src/NativeImpl/LunaHook/LunaHook/enginecollection32.cpp", '#include "engine32/ApRicoT.h"', '#include "engine32/ApricoT.h"'),
        ("src/NativeImpl/LunaHook/LunaHook/enginecollection32.cpp", '#include "engine32/Lucifen.h"', '#include "engine32/lucifen.h"'),
    ):
        inc_path = luna / rel
        inc_text = inc_path.read_text(encoding="utf-8")
        if inc_text.count(old_inc) != 1:
            raise ValueError(f"Cross-compile case-fix patch no longer matches {rel}")
        inc_path.write_text(inc_text.replace(old_inc, new_inc), encoding="utf-8")

    types_h = luna / "src/NativeImpl/LunaHook/include/types.h"
    th_text = types_h.read_text(encoding="utf-8")
    th_old = "template <typename StringT, typename = std::enable_if_t<!std::is_pointer_v<StringT>>>"
    th_new = "template <typename StringT, typename = std::enable_if_t<!std::is_pointer_v<StringT> && !std::is_array_v<StringT>>>"
    if th_text.count(th_old) != 1:
        raise ValueError("Cross-compile types.h patch no longer matches the locked LunaTranslator source")
    types_h.write_text(th_text.replace(th_old, th_new), encoding="utf-8")

    unicorn = luna / "src/NativeImpl/LunaHook/LunaHook/engine32/Unicorn.cpp"
    uc_text = unicorn.read_text(encoding="utf-8")
    uc_old = "        enum\n        {\n          role = Engine::OtherRole\n        };"
    uc_new = "        // enum { role = Engine::OtherRole };"
    if uc_text.count(uc_old) != 1:
        raise ValueError("Cross-compile Unicorn.cpp patch no longer matches the locked LunaTranslator source")
    unicorn.write_text(uc_text.replace(uc_old, uc_new), encoding="utf-8")

    print(f"Prepared {len(packages)} pinned native source trees: {output}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        prepare(args.output.resolve())
    except (KeyError, OSError, tarfile.TarError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
