// SPDX-License-Identifier: GPL-3.0-or-later
#include <windows.h>

namespace {
constexpr wchar_t kWindowClass[] = L"MaltoriaUnicodeFixtureWindow";
constexpr wchar_t kWindowTitle[] = L"Maltoria Unicode Fixture";
constexpr wchar_t kFixtureText[] = L"こんにちは、世界。雪国 — Maltoria text fixture";

LRESULT CALLBACK window_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    if (message == WM_PAINT) {
        PAINTSTRUCT paint{};
        HDC context = BeginPaint(window, &paint);
        SetBkMode(context, TRANSPARENT);
        TextOutW(context, 24, 24, kFixtureText,
                 static_cast<int>(sizeof(kFixtureText) / sizeof(kFixtureText[0]) - 1));
        EndPaint(window, &paint);
        return 0;
    }
    if (message == WM_DESTROY) {
        PostQuitMessage(0);
        return 0;
    }
    if (message == WM_TIMER && wparam == 1) {
        KillTimer(window, 1);
        DestroyWindow(window);
        return 0;
    }
    if (message == WM_TIMER && wparam == 2) {
        InvalidateRect(window, nullptr, FALSE);
        return 0;
    }
    return DefWindowProcW(window, message, wparam, lparam);
}
} // namespace

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE, PWSTR, int show_command) {
    WNDCLASSEXW window_class{};
    window_class.cbSize = sizeof(window_class);
    window_class.hInstance = instance;
    window_class.lpfnWndProc = window_proc;
    window_class.lpszClassName = kWindowClass;
    window_class.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    if (!RegisterClassExW(&window_class)) return 1;

    HWND window = CreateWindowExW(0, kWindowClass, kWindowTitle, WS_OVERLAPPEDWINDOW,
                                  CW_USEDEFAULT, CW_USEDEFAULT, 640, 180,
                                  nullptr, nullptr, instance, nullptr);
    if (!window) return 2;
    SetTimer(window, 1, 180000, nullptr);
    SetTimer(window, 2, 350, nullptr);
    ShowWindow(window, show_command);
    UpdateWindow(window);

    MSG message{};
    while (GetMessageW(&message, nullptr, 0, 0) > 0) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    return static_cast<int>(message.wParam);
}
