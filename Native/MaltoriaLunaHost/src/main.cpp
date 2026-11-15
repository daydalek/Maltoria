// SPDX-License-Identifier: GPL-3.0-or-later
#include <winsock2.h>
#include <windows.h>
#include <tlhelp32.h>

#include <nlohmann/json.hpp>

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>
#include <deque>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

namespace {
using Json = nlohmann::json;
constexpr std::size_t kMaxFrameBytes = 64 * 1024;
constexpr std::size_t kMaxTokenBytes = 4096;
constexpr std::size_t kMaxSessionBytes = 128;
constexpr std::size_t kMaxRequestIDBytes = 128;
constexpr std::size_t kMaxImagePathBytes = 8192;
constexpr std::size_t kMaxProcessCount = 512;
constexpr DWORD kConnectTimeoutMs = 5000;
constexpr DWORD kSendTimeoutMs = 5000;
constexpr DWORD kReceiveTimeoutMs = 30000;

enum class ReadResult { frame, clean_eof, invalid };

class WinsockLifetime {
public:
    WinsockLifetime() { valid_ = WSAStartup(MAKEWORD(2, 2), &data_) == 0; }
    ~WinsockLifetime() { if (valid_) WSACleanup(); }
    bool valid() const { return valid_; }
private:
    WSADATA data_{};
    bool valid_ = false;
};

class SocketLifetime {
public:
    explicit SocketLifetime(SOCKET socket = INVALID_SOCKET) : socket_(socket) {}
    ~SocketLifetime() { if (socket_ != INVALID_SOCKET) closesocket(socket_); }
    SocketLifetime(const SocketLifetime&) = delete;
    SocketLifetime& operator=(const SocketLifetime&) = delete;
    SOCKET get() const { return socket_; }
private:
    SOCKET socket_;
};

class HandleLifetime {
public:
    explicit HandleLifetime(HANDLE handle = nullptr) : handle_(handle) {}
    ~HandleLifetime() { if (handle_ && handle_ != INVALID_HANDLE_VALUE) CloseHandle(handle_); }
    HandleLifetime(const HandleLifetime&) = delete;
    HandleLifetime& operator=(const HandleLifetime&) = delete;
    HANDLE get() const { return handle_; }
private:
    HANDLE handle_;
};

bool ascii_value(const char* name, std::size_t maximum, std::string& value) {
    const char* raw = std::getenv(name);
    if (!raw) return false;
    value.assign(raw);
    if (value.empty() || value.size() > maximum) return false;
    return std::all_of(value.begin(), value.end(), [](unsigned char c) {
        return c >= 0x21 && c <= 0x7e;
    });
}

bool parse_port(const char* raw, unsigned short& port) {
    if (!raw || !*raw) return false;
    unsigned long value = 0;
    for (const unsigned char c : std::string(raw)) {
        if (c < '0' || c > '9') return false;
        value = value * 10 + static_cast<unsigned long>(c - '0');
        if (value > 65535) return false;
    }
    if (value == 0) return false;
    port = static_cast<unsigned short>(value);
    return true;
}

bool set_timeouts(SOCKET socket) {
    const DWORD send_timeout = kSendTimeoutMs;
    const DWORD receive_timeout = kReceiveTimeoutMs;
    return setsockopt(socket, SOL_SOCKET, SO_SNDTIMEO,
                      reinterpret_cast<const char*>(&send_timeout), sizeof(send_timeout)) == 0
        && setsockopt(socket, SOL_SOCKET, SO_RCVTIMEO,
                      reinterpret_cast<const char*>(&receive_timeout), sizeof(receive_timeout)) == 0;
}

SOCKET connect_loopback(unsigned short port) {
    SOCKET socket = ::socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (socket == INVALID_SOCKET) return INVALID_SOCKET;
    u_long nonblocking = 1;
    if (ioctlsocket(socket, FIONBIO, &nonblocking) != 0) {
        closesocket(socket);
        return INVALID_SOCKET;
    }

    sockaddr_in address{};
    address.sin_family = AF_INET;
    address.sin_port = htons(port);
    address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    int connected = ::connect(socket, reinterpret_cast<const sockaddr*>(&address), sizeof(address));
    if (connected == SOCKET_ERROR) {
        const int error = WSAGetLastError();
        if (error != WSAEWOULDBLOCK && error != WSAEINPROGRESS) {
            closesocket(socket);
            return INVALID_SOCKET;
        }
        fd_set writable;
        FD_ZERO(&writable);
        FD_SET(socket, &writable);
        timeval timeout{};
        timeout.tv_sec = static_cast<long>(kConnectTimeoutMs / 1000);
        if (select(0, nullptr, &writable, nullptr, &timeout) != 1) {
            closesocket(socket);
            return INVALID_SOCKET;
        }
        int connect_error = 0;
        int option_length = sizeof(connect_error);
        if (getsockopt(socket, SOL_SOCKET, SO_ERROR,
                       reinterpret_cast<char*>(&connect_error), &option_length) != 0
            || connect_error != 0) {
            closesocket(socket);
            return INVALID_SOCKET;
        }
    }

    u_long blocking = 0;
    if (ioctlsocket(socket, FIONBIO, &blocking) != 0 || !set_timeouts(socket)) {
        closesocket(socket);
        return INVALID_SOCKET;
    }
    return socket;
}

bool send_bytes(SOCKET socket, const char* bytes, std::size_t size) {
    std::size_t sent = 0;
    while (sent < size) {
        const int chunk = static_cast<int>(std::min<std::size_t>(size - sent, 1 << 20));
        const int result = send(socket, bytes + sent, chunk, 0);
        if (result == SOCKET_ERROR || result == 0) return false;
        sent += static_cast<std::size_t>(result);
    }
    return true;
}

bool send_frame(SOCKET socket, const Json& value) {
    std::string frame;
    try {
        frame = value.dump(-1, ' ', false, Json::error_handler_t::strict);
    } catch (...) {
        return false;
    }
    if (frame.size() + 1 > kMaxFrameBytes) return false;
    frame.push_back('\n');
    return send_bytes(socket, frame.data(), frame.size());
}

ReadResult receive_frame(SOCKET socket, std::string& pending, std::string& frame) {
    for (;;) {
        const std::size_t newline = pending.find('\n');
        if (newline != std::string::npos) {
            if (newline + 1 > kMaxFrameBytes) return ReadResult::invalid;
            frame.assign(pending.data(), newline);
            pending.erase(0, newline + 1);
            if (!frame.empty() && frame.back() == '\r') frame.pop_back();
            return frame.empty() ? ReadResult::invalid : ReadResult::frame;
        }
        if (pending.size() >= kMaxFrameBytes) return ReadResult::invalid;
        std::array<char, 4096> buffer{};
        const int received = recv(socket, buffer.data(), static_cast<int>(buffer.size()), 0);
        if (received == 0) return pending.empty() ? ReadResult::clean_eof : ReadResult::invalid;
        if (received == SOCKET_ERROR) return ReadResult::invalid;
        pending.append(buffer.data(), static_cast<std::size_t>(received));
    }
}

bool parse_object(const std::string& frame, Json& message) {
    message = Json::parse(frame, nullptr, false);
    return !message.is_discarded() && message.is_object();
}

bool string_field(const Json& value, const char* key, std::size_t maximum, std::string& output) {
    const auto found = value.find(key);
    if (found == value.end() || !found->is_string()) return false;
    output = found->get<std::string>();
    return !output.empty() && output.size() <= maximum;
}

bool version_one(const Json& value) {
    const auto version = value.find("version");
    if (version == value.end() || !version->is_number_integer()) return false;
    try { return version->get<std::int64_t>() == 1; }
    catch (...) { return false; }
}

std::string wide_to_utf8(const wchar_t* value, int length) {
    if (!value || length <= 0) return {};
    const int required = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value, length,
                                             nullptr, 0, nullptr, nullptr);
    if (required <= 0) return {};
    std::string output(static_cast<std::size_t>(required), '\0');
    if (WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value, length,
                            output.data(), required, nullptr, nullptr) != required) return {};
    return output;
}

std::string machine_architecture(USHORT machine) {
    switch (machine) {
        case IMAGE_FILE_MACHINE_I386: return "x86";
        case IMAGE_FILE_MACHINE_AMD64: return "x64";
        case IMAGE_FILE_MACHINE_ARM64: return "arm64";
        default: return "unknown";
    }
}

std::string process_architecture(HANDLE process) {
    using IsWow64Process2Function = BOOL(WINAPI*)(HANDLE, USHORT*, USHORT*);
    const auto kernel = GetModuleHandleW(L"kernel32.dll");
    const auto query = reinterpret_cast<IsWow64Process2Function>(
        GetProcAddress(kernel, "IsWow64Process2"));
    if (query) {
        USHORT process_machine = IMAGE_FILE_MACHINE_UNKNOWN;
        USHORT native_machine = IMAGE_FILE_MACHINE_UNKNOWN;
        if (!query(process, &process_machine, &native_machine)) return "unknown";
        return machine_architecture(process_machine == IMAGE_FILE_MACHINE_UNKNOWN
                                         ? native_machine : process_machine);
    }

    BOOL wow64 = FALSE;
    if (!IsWow64Process(process, &wow64)) return "unknown";
    SYSTEM_INFO system{};
    GetNativeSystemInfo(&system);
    if (system.wProcessorArchitecture == PROCESSOR_ARCHITECTURE_AMD64)
        return wow64 ? "x86" : "x64";
    if (system.wProcessorArchitecture == PROCESSOR_ARCHITECTURE_INTEL) return "x86";
    return "unknown";
}

std::string creation_time_hex(const FILETIME& creation) {
    ULARGE_INTEGER ticks{};
    ticks.LowPart = creation.dwLowDateTime;
    ticks.HighPart = creation.dwHighDateTime;
    char buffer[17]{};
    std::snprintf(buffer, sizeof(buffer), "%016llX",
                  static_cast<unsigned long long>(ticks.QuadPart));
    return buffer;
}

bool process_record_from_handle(HANDLE process, DWORD pid, Json& record) {
    std::vector<wchar_t> image(32768);
    DWORD image_length = static_cast<DWORD>(image.size());
    if (!QueryFullProcessImageNameW(process, 0, image.data(), &image_length)) return false;
    const std::string path = wide_to_utf8(image.data(), static_cast<int>(image_length));
    if (path.empty() || path.size() > kMaxImagePathBytes) return false;
    FILETIME creation{}, exit{}, kernel{}, user{};
    if (!GetProcessTimes(process, &creation, &exit, &kernel, &user)) return false;
    record = Json{
        {"guestPID", pid},
        {"exePath", path},
        {"arch", process_architecture(process)},
        {"creationTime", creation_time_hex(creation)}
    };
    return true;
}

bool process_record(DWORD pid, Json& record) {
    HandleLifetime process(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid));
    return process.get() && process_record_from_handle(process.get(), pid, record);
}

Json make_response(const std::string& id, const std::string& session,
                   bool ok, const Json& body) {
    Json response{
        {"type", "response"}, {"id", id}, {"session", session}, {"ok", ok}
    };
    if (ok) response["result"] = body;
    else response["error"] = body;
    return response;
}

Json list_processes(const std::string& id, const std::string& session) {
    Json records = Json::array();
    bool truncated = false;
    HandleLifetime snapshot(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0));
    if (snapshot.get() == INVALID_HANDLE_VALUE) {
        return make_response(id, session, false,
                             Json{{"code", "process_list_failed"}, {"message", "Process listing failed."}});
    }

    PROCESSENTRY32W entry{};
    entry.dwSize = sizeof(entry);
    if (!Process32FirstW(snapshot.get(), &entry)) {
        return make_response(id, session, false,
                             Json{{"code", "process_list_failed"}, {"message", "Process listing failed."}});
    }

    do {
        Json record;
        if (!process_record(entry.th32ProcessID, record)) {
            truncated = true;
            continue;
        }
        Json candidate_records = records;
        candidate_records.push_back(record);
        Json candidate = make_response(id, session, true,
                                       Json{{"processes", candidate_records}, {"truncated", true}});
        if (candidate.dump().size() + 1 > kMaxFrameBytes || records.size() >= kMaxProcessCount) {
            truncated = true;
            break;
        }
        records.push_back(std::move(record));
    } while (Process32NextW(snapshot.get(), &entry));

    return make_response(id, session, true,
                         Json{{"processes", std::move(records)}, {"truncated", truncated}});
}

#include "capture.inc"

bool valid_ack(const Json& ack, const std::string& session) {
    std::string type;
    std::string response_session;
    const auto ok = ack.find("ok");
    return string_field(ack, "type", 16, type) && type == "hello"
        && version_one(ack)
        && string_field(ack, "session", kMaxSessionBytes, response_session)
        && response_session == session
        && ok != ack.end() && ok->is_boolean() && ok->get<bool>();
}

int run() {
    std::string token;
    std::string session;
    unsigned short port = 0;
    if (!ascii_value("MALTORIA_HOOK_TOKEN", kMaxTokenBytes, token)
        || !ascii_value("MALTORIA_HOOK_SESSION", kMaxSessionBytes, session)
        || !parse_port(std::getenv("MALTORIA_HOOK_PORT"), port)) return 2;

    WinsockLifetime winsock;
    if (!winsock.valid()) return 3;
    SocketLifetime socket(connect_loopback(port));
    if (socket.get() == INVALID_SOCKET) return 4;

    const Json hello{
        {"type", "hello"}, {"version", 1}, {"token", token},
        {"session", session}, {"arch", MALTORIA_ARCH}
    };
    if (!send_frame(socket.get(), hello)) return 5;
    std::string pending;
    std::string frame;
    Json acknowledgement;
    if (receive_frame(socket.get(), pending, frame) != ReadResult::frame
        || !parse_object(frame, acknowledgement)
        || !valid_ack(acknowledgement, session)) return 6;

    // Upstream callbacks may outlive a detach. Keep their state valid until the
    // helper process exits; shutdown disables the queue before closing sockets.
    auto* capture = new CaptureSession(socket.get(), session);
    auto finish = [&](int code) {
        capture->stop();
        return code;
    };
    for (;;) {
        Json request;
        if (receive_frame(socket.get(), pending, frame) != ReadResult::frame
            || !parse_object(frame, request)) return finish(0);
        std::string type, request_session, id, command;
        if (!string_field(request, "type", 16, type) || type != "command"
            || !string_field(request, "session", kMaxSessionBytes, request_session)
            || request_session != session
            || !string_field(request, "id", kMaxRequestIDBytes, id)
            || !string_field(request, "command", 64, command)) return finish(7);

        Json response;
        if (command == "listProcesses") {
            response = list_processes(id, session);
        } else if (command == "shutdown") {
            capture->reply(make_response(id, session, true, Json::object()));
            return finish(0);
        } else if (command == "ping") {
            response = make_response(id, session, true, Json::object());
        } else if (command == "attach" || command == "detach" || command == "insertHook") {
            response = capture->command(id, command, request.value("params", Json::object()));
        } else {
            const Json error{
                {"code", "unknown_command"}, {"message", "Command is not supported."}
            };
            response = make_response(id, session, false, error);
        }
        if (!capture->reply(std::move(response))) return finish(8);
    }
}
} // namespace

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR, int) {
    try { return run(); }
    catch (...) { return 11; } // Never print request, token, or dialogue data.
}
