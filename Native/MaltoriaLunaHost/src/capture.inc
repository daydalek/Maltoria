// SPDX-License-Identifier: GPL-3.0-or-later
// Included inside main.cpp's private namespace. C ABI declarations match
// LunaTranslator 178b4372/src/NativeImpl/LunaHook/LunaHost/LunaHostDll.cpp.
// Preserve the upstream DLL for its own thread/pipe lifecycle implementation.
struct LunaThreadParam {
    DWORD processId;
    std::uint64_t addr;
    std::uint64_t ctx;
    std::uint64_t ctx2;
};
static_assert(sizeof(LunaThreadParam) == 32 && offsetof(LunaThreadParam, addr) == 8);

using ProcessEvent = void (*)(DWORD);
using ThreadCreated = void (*)(const wchar_t*, const char*, LunaThreadParam, bool);
using ThreadEvent = void (*)(const wchar_t*, const char*, LunaThreadParam);
using TextEvent = void (*)(const wchar_t*, const char*, LunaThreadParam, const wchar_t*);
using StartHost = void (*)(ProcessEvent, ProcessEvent, ThreadCreated, ThreadEvent,
    TextEvent, void (*)(int, const wchar_t*), void (*)(DWORD, std::uint64_t, const wchar_t*),
    void (*)(const wchar_t*, LunaThreadParam), wchar_t* (*)(const wchar_t*),
    void (*)(const wchar_t*, const wchar_t*, const wchar_t*));

std::string hex_address(std::uint64_t address) {
    char buffer[19]{};
    std::snprintf(buffer, sizeof(buffer), "0x%016llX", static_cast<unsigned long long>(address));
    return buffer;
}

std::wstring utf8_to_wide(const std::string& text) {
    const int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
        static_cast<int>(text.size()), nullptr, 0);
    if (count <= 0) return {};
    std::wstring result(static_cast<std::size_t>(count), L'\0');
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
        static_cast<int>(text.size()), result.data(), count);
    return result;
}

std::wstring payload_directory() {
    std::vector<wchar_t> path(32768);
    DWORD count = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
    if (!count || count >= path.size()) return {};
    std::wstring value(path.data(), count);
    const auto separator = value.find_last_of(L"\\/");
    return separator == std::wstring::npos ? L"" : value.substr(0, separator + 1);
}

// Only this writer touches the socket after authentication. Callbacks never
// wait for socket I/O: bounded enqueue under mutex, with observable overflow counters.
class CaptureOutbox {
public:
    CaptureOutbox(SOCKET socket, std::string session)
        : socket_(socket), session_(std::move(session)), writer_([this] { write(); }) {}

    void event(Json value) noexcept {
        if (closed_) return;
        try {
            std::lock_guard lock(mutex_);
            if (events_.size() >= 2048) {
                ++dropped_;
            } else {
                value["session"] = session_;
                events_.push_back(std::move(value));
            }
            ready_.notify_one();
        } catch (...) { ++dropped_; }
    }

    bool reply(Json value) {
        std::lock_guard lock(mutex_);
        if (closed_ || replies_.size() >= 32) return false;
        replies_.push_back(std::move(value));
        ready_.notify_one();
        return true;
    }

    void dropped() noexcept { ++dropped_; ready_.notify_one(); }

    void close() {
        {
            std::lock_guard lock(mutex_);
            closed_ = true;
            events_.clear();
        }
        ready_.notify_one();
        if (writer_.joinable()) writer_.join();
    }

private:
    void write() noexcept {
        try {
            for (;;) {
                Json value;
                {
                    std::unique_lock lock(mutex_);
                    ready_.wait(lock, [&] {
                        return closed_ || !replies_.empty() || !events_.empty() || dropped_ != 0;
                    });
                    if (!replies_.empty()) {
                        value = std::move(replies_.front()); replies_.pop_front();
                    } else if (closed_) {
                        return;
                    } else if (const auto lost = dropped_.exchange(0); lost != 0) {
                        value = {{"type", "error"}, {"session", session_},
                                 {"code", "events_dropped"}, {"dropped", lost}};
                    } else {
                        value = std::move(events_.front()); events_.pop_front();
                    }
                }
                // Reject, never silently truncate dialogue that exceeds a frame.
                if (value.dump().size() + 1 > kMaxFrameBytes) {
                    value = {{"type", "error"}, {"session", session_}, {"code", "frame_too_large"}};
                }
                if (!send_frame(socket_, value)) break;
            }
        } catch (...) {}
        closed_ = true;
        ::shutdown(socket_, SD_BOTH); // Wake the reader on a broken writer.
    }

    SOCKET socket_;
    const std::string session_;
    std::mutex mutex_;
    std::condition_variable ready_;
    std::deque<Json> replies_, events_;
    std::atomic<bool> closed_{false};
    std::atomic<std::uint64_t> dropped_{0};
    std::thread writer_;
};

class CaptureSession {
public:
    CaptureSession(SOCKET socket, const std::string& session)
        : session_(session), output_(socket, session) { current_ = this; }

    bool reply(Json value) { return output_.reply(std::move(value)); }

    Json command(const std::string& id, const std::string& name, const Json& params) {
        if (!params.is_object()) return failure(id, "invalid_params");
        try {
            if (name == "attach") return attach(id, params);
            if (name == "detach") {
                const bool detached = detach(3000);
                return detached ? success(id) : failure(id, "detach_timeout");
            }
            std::string code;
            if (!active_pid_ || !connected_) return failure(id, "not_attached");
            if (!string_field(params, "hookCode", 2048, code) || code.find('\0') != std::string::npos)
                return failure(id, "invalid_hook_code");
            const auto wide = utf8_to_wide(code);
            if (wide.empty()) return failure(id, "invalid_hook_code");
            insert_(active_pid_, wide.c_str());
            return success(id);
        } catch (...) { return failure(id, "capture_failed"); }
    }

    void stop() {
        detach(1500);
        stopped_ = true;
        output_.close();
    }

private:
    Json success(const std::string& id, Json body = Json::object()) {
        return make_response(id, session_, true, body);
    }
    Json failure(const std::string& id, const char* code) {
        return make_response(id, session_, false,
            {{"code", code}, {"message", "Text capture operation failed; see the error code."}});
    }

    bool load() {
        if (start_) return true;
        const auto directory = payload_directory();
        if (directory.empty()) return false;
        const auto suffix = sizeof(void*) == 8 ? L"64.dll" : L"32.dll";
        hook_path_ = directory + L"LunaHook" + suffix;
        if (GetFileAttributesW(hook_path_.c_str()) == INVALID_FILE_ATTRIBUTES) return false;
        HMODULE library = LoadLibraryExW((directory + L"LunaHost" + suffix).c_str(), nullptr,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
        if (!library) return false;
        auto start = reinterpret_cast<StartHost>(GetProcAddress(library, "Luna_Start"));
        connect_ = reinterpret_cast<void (*)(DWORD)>(GetProcAddress(library, "Luna_ConnectProcess"));
        detach_ = reinterpret_cast<void (*)(DWORD)>(GetProcAddress(library, "Luna_DetachProcess"));
        check_ = reinterpret_cast<bool (*)(DWORD)>(GetProcAddress(library, "Luna_CheckIfNeedInject"));
        insert_ = reinterpret_cast<void (*)(DWORD, const wchar_t*)>(GetProcAddress(library, "Luna_InsertHookCode"));
        auto settings = reinterpret_cast<void (*)(int, int, int, int, bool)>(
            GetProcAddress(library, "Luna_Settings"));
        if (!start || !connect_ || !detach_ || !check_ || !insert_ || !settings) {
            FreeLibrary(library);
            return false;
        }
        // flushDelay = 0 flushes FULL_STRING hooks immediately instead of concatenating
        // fast-forwarded dialogue lines across calls; debouncing is handled by the host bridge.
        settings(0, 932, 10000, 1000, true);
        start(on_connect, on_disconnect, on_created, on_removed, on_text,
              on_info, on_insert, on_embed, on_i18n, on_game_info);
        // Do not unload a DLL whose worker threads may still invoke callbacks.
        start_ = start;
        return true;
    }

    // Resolve LoadLibraryW in the corresponding target module, accounting for
    // forwarded exports and per-process ASLR; never assume equal module bases.
    LPTHREAD_START_ROUTINE remote_loader(DWORD pid) {
        auto local = GetProcAddress(GetModuleHandleW(L"kernel32.dll"), "LoadLibraryW");
        HMODULE owner = nullptr;
        if (!local || !GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(local), &owner)) return nullptr;
        wchar_t path[32768]{};
        if (!GetModuleFileNameW(owner, path, 32768)) return nullptr;
        const wchar_t* name = wcsrchr(path, L'\\');
        name = name ? name + 1 : path;
        const auto offset = reinterpret_cast<std::uintptr_t>(local) - reinterpret_cast<std::uintptr_t>(owner);
        HandleLifetime snapshot(CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid));
        if (snapshot.get() == INVALID_HANDLE_VALUE) return nullptr;
        MODULEENTRY32W module{};
        module.dwSize = sizeof(module);
        if (!Module32FirstW(snapshot.get(), &module)) return nullptr;
        do {
            if (_wcsicmp(module.szModule, name) == 0 && offset < module.modBaseSize) {
                return reinterpret_cast<LPTHREAD_START_ROUTINE>(module.modBaseAddr + offset);
            }
        } while (Module32NextW(snapshot.get(), &module));
        return nullptr;
    }

    const char* inject(HANDLE process, DWORD pid) {
        auto loader = remote_loader(pid);
        if (!loader) return "loader_unavailable";
        const SIZE_T size = (hook_path_.size() + 1) * sizeof(wchar_t);
        void* remote = VirtualAllocEx(process, nullptr, size, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE);
        if (!remote) return "allocation_failed";
        SIZE_T written = 0;
        if (!WriteProcessMemory(process, remote, hook_path_.c_str(), size, &written) || written != size) {
            VirtualFreeEx(process, remote, 0, MEM_RELEASE);
            return "dll_path_write_failed";
        }
        HandleLifetime thread(CreateRemoteThread(process, nullptr, 0, loader, remote, 0, nullptr));
        if (!thread.get()) {
            VirtualFreeEx(process, remote, 0, MEM_RELEASE);
            return "load_thread_failed";
        }
        const DWORD wait = WaitForSingleObject(thread.get(), 10000);
        if (wait != WAIT_OBJECT_0) {
            // The loader may still read its argument. Leave this bounded buffer
            // allocated rather than free live memory or terminate a game thread.
            return "load_timeout";
        }
        VirtualFreeEx(process, remote, 0, MEM_RELEASE);
        DWORD result = 0;
        if (!GetExitCodeThread(thread.get(), &result) || (!result && !connected_)) return "dll_load_failed";
        return nullptr;
    }

    Json attach(const std::string& id, const Json& params) {
        if (owned_process_) return failure(id, "already_attached");
        auto it = params.find("target");
        if (it == params.end() || !it->is_object()) return failure(id, "invalid_target");
        const auto& target = *it;
        std::string path, creation, arch;
        auto pid_value = target.find("guestPID");
        if (pid_value == target.end() || !pid_value->is_number_unsigned()
            || pid_value->get<std::uint64_t>() == 0 || pid_value->get<std::uint64_t>() > MAXDWORD
            || !string_field(target, "exePath", kMaxImagePathBytes, path)
            || !string_field(target, "creationTime", 16, creation) || creation.size() != 16
            || !string_field(target, "arch", 8, arch)) return failure(id, "invalid_target");
        const DWORD pid = pid_value->get<DWORD>();
        if (pid == GetCurrentProcessId()) return failure(id, "invalid_target");
        HandleLifetime process(OpenProcess(PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION |
            PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ | SYNCHRONIZE, FALSE, pid));
        if (!process.get()) return failure(id, "process_unavailable");
        Json actual;
        if (WaitForSingleObject(process.get(), 0) != WAIT_TIMEOUT ||
            !process_record_from_handle(process.get(), pid, actual)) return failure(id, "process_exited");
        if (actual["exePath"] != path || actual["creationTime"] != creation || actual["arch"] != arch)
            return failure(id, "stale_identity");
        if (arch != MALTORIA_ARCH) return failure(id, "architecture_mismatch");
        if (!load()) return failure(id, "payload_unavailable");
        if (!check_(pid)) return failure(id, "already_hooked");
        HANDLE retained = nullptr;
        if (!DuplicateHandle(GetCurrentProcess(), process.get(), GetCurrentProcess(), &retained,
                0, FALSE, DUPLICATE_SAME_ACCESS)) return failure(id, "process_unavailable");
        owned_process_ = retained;
        active_pid_ = pid;
        connected_ = false;
        connect_(pid);
        if (const char* error = inject(process.get(), pid)) return failure(id, error);
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        while (!connected_ && std::chrono::steady_clock::now() < deadline) {
            if (WaitForSingleObject(process.get(), 100) != WAIT_TIMEOUT) return failure(id, "process_exited");
        }
        if (!connected_) return failure(id, "hook_connect_timeout");
        Json after;
        if (WaitForSingleObject(process.get(), 0) != WAIT_TIMEOUT ||
            !process_record_from_handle(process.get(), pid, after) || after != actual)
            return failure(id, "stale_identity");
        return success(id, {{"target", actual}});
    }

    bool detach(DWORD timeout) {
        if (!owned_process_) return true;
        const DWORD pid = active_pid_;
        if (pid && detach_) { try { detach_(pid); } catch (...) {} }
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(timeout);
        while (connected_ && std::chrono::steady_clock::now() < deadline) {
            if (WaitForSingleObject(owned_process_, 20) != WAIT_TIMEOUT) break;
        }
        const bool detached = !connected_ || WaitForSingleObject(owned_process_, 0) != WAIT_TIMEOUT;
        active_pid_ = 0;
        connected_ = false;
        CloseHandle(owned_process_);
        owned_process_ = nullptr;
        return detached;
    }

    static Json thread_record(const wchar_t* code, const char* name, LunaThreadParam param) {
        const auto length = code ? wcsnlen_s(code, 2048) : 0;
        std::string hook_name(name ? name : "", name ? strnlen_s(name, 512) : 0);
        if (!hook_name.empty() && utf8_to_wide(hook_name).empty()) hook_name = "Hook";
        return {{"guestPID", param.processId}, {"addressHex", hex_address(param.addr)},
            {"contextHex", hex_address(param.ctx)}, {"context2Hex", hex_address(param.ctx2)},
            {"hookCode", wide_to_utf8(code, static_cast<int>(length))}, {"hookName", hook_name}};
    }
    static bool accept(DWORD pid) { return current_ && !current_->stopped_ && current_->active_pid_ == pid; }
    static void on_connect(DWORD pid) {
        if (!accept(pid)) return;
        current_->connected_ = true;
        current_->output_.event({{"type", "process"}, {"action", "connected"}, {"guestPID", pid}});
    }
    static void on_disconnect(DWORD pid) {
        if (!accept(pid)) return;
        current_->connected_ = false;
        current_->output_.event({{"type", "process"}, {"action", "disconnected"}, {"guestPID", pid}});
    }
    static void on_created(const wchar_t* code, const char* name, LunaThreadParam param, bool) {
        if (!accept(param.processId)) return;
        try { current_->output_.event({{"type", "thread"}, {"action", "created"},
            {"thread", thread_record(code, name, param)}}); } catch (...) { current_->output_.dropped(); }
    }
    static void on_removed(const wchar_t* code, const char* name, LunaThreadParam param) {
        if (!accept(param.processId)) return;
        try { current_->output_.event({{"type", "thread"}, {"action", "removed"},
            {"thread", thread_record(code, name, param)}}); } catch (...) { current_->output_.dropped(); }
    }
    static void on_text(const wchar_t* code, const char* name, LunaThreadParam param, const wchar_t* text) {
        if (!accept(param.processId) || !text) return;
        try {
            const auto length = wcsnlen_s(text, 16385);
            if (length > 16384) { current_->output_.dropped(); return; }
            auto value = wide_to_utf8(text, static_cast<int>(length));
            if (value.empty()) return;
            current_->output_.event({{"type", "text"}, {"sequence", ++current_->sequence_},
                {"thread", thread_record(code, name, param)}, {"text", std::move(value)}});
        } catch (...) { current_->output_.dropped(); }
    }
    static void on_info(int, const wchar_t*) {} // Upstream diagnostics can contain dialogue.
    static void on_insert(DWORD, std::uint64_t, const wchar_t*) {}
    static void on_embed(const wchar_t*, LunaThreadParam) {}
    static wchar_t* on_i18n(const wchar_t*) { return nullptr; }
    static void on_game_info(const wchar_t*, const wchar_t*, const wchar_t*) {}

    inline static CaptureSession* current_ = nullptr;
    const std::string session_;
    CaptureOutbox output_;
    std::wstring hook_path_;
    StartHost start_ = nullptr;
    void (*connect_)(DWORD) = nullptr;
    void (*detach_)(DWORD) = nullptr;
    bool (*check_)(DWORD) = nullptr;
    void (*insert_)(DWORD, const wchar_t*) = nullptr;
    HANDLE owned_process_ = nullptr;
    std::atomic<DWORD> active_pid_{0};
    std::atomic<bool> connected_{false}, stopped_{false};
    std::atomic<std::uint64_t> sequence_{0};
};
