// ==WindhawkMod==
// @id              cimori-tray-bridge
// @name            Cimori tray bridge
// @description     Passes the tray icons apps give the taskbar to the Cimori island, so it can show and click them
// @version         1.2
// @author          Marcus Paixão
// @github          https://github.com/Vini-Paixao
// @include         explorer.exe
// @compilerOptions -lcomctl32 -lole32 -loleaut32 -lruntimeobject
// @license         GPL-3.0
// ==/WindhawkMod==

// ==WindhawkModReadme==
/*
# Cimori tray bridge

Optional companion of the Cimori island (Cimori.exe). Apps hand their tray
icons to the taskbar, inside explorer.exe; another program can't get them
from there. This mod reads them as they pass (like Mac Island & Dock does)
and hands them to the island, which shows them and clicks them the way the
taskbar does: left click is the app's action, right click its own menu.

Without the island running it does nothing but keep the list. The taskbar
still gets every icon as before.
*/
// ==/WindhawkModReadme==

// SPDX-License-Identifier: GPL-3.0
// Ported from Mac Island & Dock 3.1.1 (caliberda, GPL-3.0): NotifyIconData32,
// TrayNotifyData, OnTrayNotify, TrayIconKey, ProcessImagePathById,
// SendTrayIcons, RequestTrayIcons and the taskbar subclass. Only the tray part;
// it sends to the Cimori island's window instead of the mod's own.

#include <windhawk_utils.h>

#include <algorithm>
#include <atomic>
#include <memory>
#include <string>
#include <vector>
#include <cmath>
#include <functional>
#undef GetCurrentTime
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.UI.Xaml.h>
#include <winrt/Windows.UI.Xaml.Controls.Primitives.h>
#include <winrt/Windows.UI.Xaml.Media.h>

// What the island and this bridge tell each other (WM_COPYDATA's dwData).
// The island's window class, and its kinds: keep in sync with
// crates/isle-platform-win/src/tray/bridge.rs.
constexpr WCHAR kHostClassName[] = L"CimoriTrayHost";
constexpr ULONG_PTR kIpcTrayIcons = 0x43494D01;  // bridge -> island
constexpr ULONG_PTR kIpcHello = 0x43494D02;      // island -> bridge
constexpr UINT kIpcTimeoutMs = 400;


// Port of FindDescendant/GetTaskbarXamlRoot, ProgressBarWatch, ProgressBarState,
// OnWindowProgress and PublishProgress (Mac Island & Dock 3.1.1, caliberda).
// All XAML objects and revokers are confined to Explorer's taskbar thread.
using namespace winrt::Windows::UI::Xaml;
using namespace winrt::Windows::UI::Xaml::Controls;
FrameworkElement FindDescendant(
    FrameworkElement root,
    std::function<bool(FrameworkElement const&)> predicate,
    int maxDepth = 12) {
    std::vector<FrameworkElement> level{root};
    for (int depth = 0; depth < maxDepth && !level.empty(); depth++) {
        std::vector<FrameworkElement> next;
        for (const auto& element : level) {
            const int count = Media::VisualTreeHelper::GetChildrenCount(element);
            for (int i = 0; i < count; i++) {
                auto child = Media::VisualTreeHelper::GetChild(element, i)
                                 .try_as<FrameworkElement>();
                if (!child) {
                    continue;
                }
                if (predicate(child)) {
                    return child;
                }
                next.push_back(child);
            }
        }
        level = std::move(next);
    }
    return nullptr;
}

FrameworkElement FindByClassName(FrameworkElement root, PCWSTR className) {
    return FindDescendant(root, [className](FrameworkElement const& element) {
        return winrt::get_class_name(element) == className;
    });
}

FrameworkElement FindByName(FrameworkElement root, PCWSTR name) {
    return FindDescendant(root, [name](FrameworkElement const& element) {
        return element.Name() == name;
    });
}

void* CTaskBand_ITaskListWndSite_vftable;
void* CSecondaryTaskBand_ITaskListWndSite_vftable;

using CTaskBand_GetTaskbarHost_t = void*(WINAPI*)(void* pThis, void** result);
CTaskBand_GetTaskbarHost_t CTaskBand_GetTaskbarHost_Original;

void* TaskbarHost_FrameHeight_Original;

using CSecondaryTaskBand_GetTaskbarHost_t = void*(WINAPI*)(void* pThis,
                                                           void** result);
CSecondaryTaskBand_GetTaskbarHost_t CSecondaryTaskBand_GetTaskbarHost_Original;

using std__Ref_count_base__Decref_t = void(WINAPI*)(void* pThis);
std__Ref_count_base__Decref_t std__Ref_count_base__Decref_Original;

// Whether taskbar.dll's symbols loaded (see ExplorerModInit). They don't
// right after a Windows update whose symbols Microsoft hasn't published yet:
// the taskbar is then left as Windows draws it (nothing above may be called,
// some of it may be unset), and what needs none still reaches the island
// (the tray icons, apps asking for attention).
bool g_taskbarSymbols = true;

XamlRoot XamlRootFromTaskbarHostSharedPtr(void* taskbarHostSharedPtr[2]) {
    if (!taskbarHostSharedPtr[0] && !taskbarHostSharedPtr[1]) {
        return nullptr;
    }

    size_t taskbarElementIUnknownOffset = 0x10;

#if defined(_M_X64)
    {
        // 48:83EC 28 | sub rsp,28
        // 48:83C1 48 | add rcx,48
        const BYTE* b = (const BYTE*)TaskbarHost_FrameHeight_Original;
        if (b[0] == 0x48 && b[1] == 0x83 && b[2] == 0xEC && b[4] == 0x48 &&
            b[5] == 0x83 && b[6] == 0xC1 && b[7] <= 0x7F) {
            taskbarElementIUnknownOffset = b[7];
        } else {
            Wh_Log(L"Unsupported TaskbarHost::FrameHeight");
        }
    }
#elif defined(_M_ARM64)
    {
        // 7f2303d5 pacibsp
        // fd7bbfa9 stp     fp, lr, [sp, #-0x10]!
        // fd030091 mov     fp, sp
        // 080c41f8 ldr     x8, [x0, #0x10]!
        const DWORD* p = (const DWORD*)TaskbarHost_FrameHeight_Original;
        if (p[0] == 0xD503237F && (p[1] & 0xFFC07FFF) == 0xA9807BFD &&
            p[2] == 0x910003FD && (p[3] & 0xFFF00FE0) == 0xF8400C00) {
            taskbarElementIUnknownOffset = (p[3] >> 12) & 0xFF;
        } else {
            Wh_Log(L"Unsupported TaskbarHost::FrameHeight");
        }
    }
#else
    // 32-bit: only the island's process (windhawk.exe) is, and the taskbar
    // part never runs there (Windows 11's explorer.exe is 64-bit).
#endif

    auto* taskbarElementIUnknown =
        *(IUnknown**)((BYTE*)taskbarHostSharedPtr[0] +
                      taskbarElementIUnknownOffset);

    FrameworkElement taskbarElement = nullptr;
    taskbarElementIUnknown->QueryInterface(winrt::guid_of<FrameworkElement>(),
                                           winrt::put_abi(taskbarElement));

    auto result = taskbarElement ? taskbarElement.XamlRoot() : nullptr;

    std__Ref_count_base__Decref_Original(taskbarHostSharedPtr[1]);

    return result;
}

XamlRoot GetTaskbarXamlRoot(HWND hTaskbarWnd) {
    if (!g_taskbarSymbols) {
        return nullptr;
    }
    HWND hTaskSwWnd = (HWND)GetProp(hTaskbarWnd, L"TaskbandHWND");
    if (!hTaskSwWnd) {
        return nullptr;
    }

    void* taskBand = (void*)GetWindowLongPtr(hTaskSwWnd, 0);
    void* taskBandForTaskListWndSite = taskBand;
    for (int i = 0; *(void**)taskBandForTaskListWndSite !=
                    CTaskBand_ITaskListWndSite_vftable;
         i++) {
        if (i == 20) {
            return nullptr;
        }
        taskBandForTaskListWndSite = (void**)taskBandForTaskListWndSite + 1;
    }

    void* taskbarHostSharedPtr[2]{};
    CTaskBand_GetTaskbarHost_Original(taskBandForTaskListWndSite,
                                      taskbarHostSharedPtr);
    return XamlRootFromTaskbarHostSharedPtr(taskbarHostSharedPtr);
}


struct ProgressBarWatch {
    winrt::weak_ref<Controls::Primitives::RangeBase> bar;
    Controls::Primitives::RangeBase::ValueChanged_revoker changed;
};
std::vector<ProgressBarWatch> g_progressBars;
std::vector<std::pair<HWND, int>> g_windowProgress;
ULONGLONG g_progressSeenAt = 0;
constexpr UINT_PTR kProgressTimer = 0x43494D03;
constexpr UINT kProgressSureSeconds = 5;
void PublishProgress();

int ProgressBarState(Controls::Primitives::RangeBase const& bar) {
    if (Media::VisualTreeHelper::GetChildrenCount(bar) > 0) {
        if (auto root = Media::VisualTreeHelper::GetChild(bar, 0).try_as<FrameworkElement>()) {
            for (const auto& group : VisualStateManager::GetVisualStateGroups(root)) {
                if (group.Name() == L"CommonStates" && group.CurrentState()) {
                    const std::wstring name = group.CurrentState().Name().c_str();
                    return name.find(L"Error") != name.npos ? 4 : name.find(L"Paused") != name.npos ? 8 : name.find(L"Indeterminate") != name.npos ? 1 : 2;
                }
            }
        }
    }
    return 2;
}
void PublishProgress() {
    const HWND taskbar = FindWindow(L"Shell_TrayWnd", nullptr);
    const HWND island = FindWindow(L"CimoriDownloadHost", nullptr);
    if (!taskbar || !g_taskbarSymbols) return;
    if (!island) { g_progressBars.clear(); g_progressSeenAt = 0; return; }
    int state = 0, value = 1000;
    UINT count = 0;
    auto rank = [](int s) { return s == 4 ? 4 : s == 8 ? 3 : s == 2 ? 2 : s == 1 ? 1 : 0; };
    std::erase_if(g_windowProgress, [](const auto& p) { return !IsWindow(p.first) || !p.second; });
    try {
        // Unlike the full dock we don't already watch every taskbar button. Traverse
        // the same visual tree here, and keep value revokers while bars are alive.
        if (auto root = (!g_windowProgress.empty() || g_progressSeenAt) ? GetTaskbarXamlRoot(taskbar) : nullptr) {
            auto content = root.Content().try_as<FrameworkElement>();
            std::vector<FrameworkElement> level;
            if (content) level.push_back(content);
            for (int depth = 0; depth < 24 && !level.empty(); depth++) {
                std::vector<FrameworkElement> next;
                for (const auto& e : level) {
                    if (e.Name() == L"ProgressIndicator") {
                        auto bar = e.try_as<Controls::Primitives::RangeBase>();
                        if (bar && bar.Visibility() == Visibility::Visible && bar.Opacity() >= .01 && bar.ActualWidth() >= 1) {
                            count++;
                            const int s = ProgressBarState(bar);
                            const double range = bar.Maximum() - bar.Minimum();
                            const int v = range > 0 ? (int)std::clamp(std::lround((bar.Value() - bar.Minimum()) * 1000 / range), 0L, 1000L) : 0;
                            if (rank(s) > rank(state)) { state = s; value = v; }
                            else if (s == state) value = std::min(value, v);
                            if (std::none_of(g_progressBars.begin(), g_progressBars.end(), [&](const auto& p) { return p.bar.get() == bar; })) {
                                ProgressBarWatch watch;
                                watch.bar = winrt::make_weak(bar);
                                watch.changed = bar.ValueChanged(winrt::auto_revoke, [](auto const&, auto const&) { PublishProgress(); });
                                g_progressBars.push_back(std::move(watch));
                            }
                        }
                    }
                    for (int i = 0, n = Media::VisualTreeHelper::GetChildrenCount(e); i < n; i++) {
                        if (auto child = Media::VisualTreeHelper::GetChild(e, i).try_as<FrameworkElement>()) next.push_back(child);
                    }
                }
                level = std::move(next);
            }
        }
    } catch (...) { state = 0; count = 0; }
    std::erase_if(g_progressBars, [](const auto& p) { return !p.bar.get(); });
    const auto now = GetTickCount64();
    if (!state) g_progressSeenAt = 0;
    else if (!g_progressSeenAt) g_progressSeenAt = now;
    if (state && now - g_progressSeenAt < kProgressSureSeconds * 1000) { state = 0; value = 0; }
    const UINT32 packet[] = {1, (UINT32)state, state == 1 ? UINT_MAX : (UINT32)value, count};
    COPYDATASTRUCT data{0x43494D03, sizeof(packet), (void*)packet};
    DWORD_PTR result;
    SendMessageTimeout(island, WM_COPYDATA, (WPARAM)taskbar, (LPARAM)&data,
        SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT, 400, &result);
}
void OnWindowProgress(HWND hwnd, int state) {
    auto it = std::find_if(g_windowProgress.begin(), g_windowProgress.end(), [&](const auto& p) { return p.first == hwnd; });
    if (it == g_windowProgress.end()) { if (state != 0) g_windowProgress.push_back({hwnd, state < 0 ? 2 : state}); }
    else if (state >= 0) it->second = state;
    PublishProgress();
}
using ProgressStateFn = void(WINAPI*)(void*, HWND, int);
using ProgressValueFn = void(WINAPI*)(void*, HWND, DWORD);
ProgressStateFn g_progressStateOriginal;
ProgressValueFn g_progressValueOriginal;
void WINAPI ProgressStateHook(void* self, HWND hwnd, int state) {
    g_progressStateOriginal(self, hwnd, state);
    OnWindowProgress(hwnd, state);
}
void WINAPI ProgressValueHook(void* self, HWND hwnd, DWORD value) {
    g_progressValueOriginal(self, hwnd, value);
    // Upstream explicitly warns that this DWORD is NOT a normalized percentage.
    OnWindowProgress(hwnd, -1);
}
bool HookProgress() {
    const auto module = LoadLibrary(L"taskbar.dll");
    if (!module) return false;
    WindhawkUtils::SYMBOL_HOOK hooks[] = {
        {{LR"(const CTaskBand::`vftable'{for `ITaskListWndSite'})"}, &CTaskBand_ITaskListWndSite_vftable},
        {{LR"(public: virtual class std::shared_ptr<class TaskbarHost> __cdecl CTaskBand::GetTaskbarHost(void)const )"}, &CTaskBand_GetTaskbarHost_Original},
        {{LR"(public: int __cdecl TaskbarHost::FrameHeight(void)const )"}, &TaskbarHost_FrameHeight_Original},
        {{LR"(public: void __cdecl std::_Ref_count_base::_Decref(void))"}, &std__Ref_count_base__Decref_Original},
        {{LR"(protected: void __cdecl CTaskBand::_OnSetProgressState(struct HWND__ * const,enum TBPFLAG))"}, &g_progressStateOriginal, ProgressStateHook},
        {{LR"(protected: void __cdecl CTaskBand::_OnSetProgressValue(struct HWND__ * const,unsigned long))"}, &g_progressValueOriginal, ProgressValueHook},
    };
    return WindhawkUtils::HookSymbols(module, hooks, ARRAYSIZE(hooks));
}

// The data the taskbar gets (with 32-bit handles, from 32 and 64-bit apps).
#pragma pack(push, 4)
struct NotifyIconData32 {
    DWORD cbSize;
    DWORD hWnd;
    UINT uID;
    UINT uFlags;
    UINT uCallbackMessage;
    DWORD hIcon;
    WCHAR szTip[128];
    DWORD dwState;
    DWORD dwStateMask;
    WCHAR szInfo[256];
    UINT uVersion;
    WCHAR szInfoTitle[64];
    DWORD dwInfoFlags;
    GUID guidItem;
    DWORD hBalloonIcon;
};
struct TrayNotifyData {
    DWORD signature;
    DWORD message;
    NotifyIconData32 nid;
};
#pragma pack(pop)
constexpr DWORD kTrayNotifySignature = 0x34753423;

// An icon handle the bridge owns, destroyed when the last copy goes.
struct OwnedIcon {
    HICON icon;
    explicit OwnedIcon(HICON icon) : icon(icon) {}
    ~OwnedIcon() {
        if (icon) {
            DestroyIcon(icon);
        }
    }
    OwnedIcon(const OwnedIcon&) = delete;
    OwnedIcon& operator=(const OwnedIcon&) = delete;
};

struct TrayIcon {
    HWND owner = nullptr;
    UINT id = 0;
    GUID guid{};
    bool hasGuid = false;
    UINT callback = 0;
    UINT version = 0;
    std::shared_ptr<OwnedIcon> icon;
    UINT serial = 0;
    std::wstring tip;
    bool hidden = false;
    DWORD processId = 0;
    // The program and the icon's ID: stays the same across restarts.
    std::wstring key;
};

// Until the app tells it (NIM_SETVERSION).
constexpr UINT kTrayVersionUnknown = UINT_MAX;

SRWLOCK g_trayIconsLock = SRWLOCK_INIT;
std::vector<TrayIcon> g_trayIcons;
std::vector<HWND> g_trayOwnersAsked;
UINT g_trayIconSerial;
std::atomic<bool> g_trayIconsSendPending;
std::atomic<bool> g_trayIconsRequested;
std::atomic<HWND> g_taskbar;
std::atomic<bool> g_unloading;

const UINT g_sendTrayIconsMessage =
    RegisterWindowMessage(L"CimoriTrayBridge_SendTrayIcons");
// SetTimer only works from the window's own thread: the subclass arms the
// progress heartbeat when this arrives there.
const UINT g_armProgressMessage =
    RegisterWindowMessage(L"CimoriTrayBridge_ArmProgress");

// A process's program file when the process can't be opened (a service's,
// or an elevated app's), as a drive path. Empty if not found.
std::wstring ProcessImagePathById(DWORD processId) {
    using NtQuerySystemInformation_t =
        LONG(NTAPI*)(ULONG, PVOID, ULONG, PULONG);
    static const auto query = (NtQuerySystemInformation_t)GetProcAddress(
        GetModuleHandle(L"ntdll.dll"), "NtQuerySystemInformation");
    constexpr ULONG kSystemProcessIdInformation = 88;
    struct {
        HANDLE processId;
        USHORT length;
        USHORT room;
        PWSTR buffer;
    } info{};
    WCHAR name[MAX_PATH * 2];
    info.processId = (HANDLE)(ULONG_PTR)processId;
    info.room = sizeof(name);
    info.buffer = name;
    if (!query ||
        query(kSystemProcessIdInformation, &info, sizeof(info), nullptr) < 0) {
        return {};
    }
    const std::wstring device(name, info.length / sizeof(WCHAR));
    WCHAR drive[] = L"A:";
    WCHAR target[MAX_PATH];
    for (WCHAR letter = L'A'; letter <= L'Z'; letter++) {
        drive[0] = letter;
        if (!QueryDosDevice(drive, target, ARRAYSIZE(target))) {
            continue;
        }
        const size_t length = wcslen(target);
        if (device.size() > length && device[length] == L'\\' &&
            device.compare(0, length, target) == 0) {
            return drive + device.substr(length);
        }
    }
    return {};
}

std::wstring TrayIconKey(const TrayIcon& icon) {
    std::wstring key;
    if (HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE,
                                     icon.processId)) {
        WCHAR path[MAX_PATH];
        DWORD length = ARRAYSIZE(path);
        if (QueryFullProcessImageName(process, 0, path, &length)) {
            key = path;
        }
        CloseHandle(process);
    }
    if (key.empty()) {
        key = ProcessImagePathById(icon.processId);
    }
    for (auto& c : key) {
        c = towlower(c);
    }
    if (icon.hasGuid) {
        WCHAR guid[64];
        StringFromGUID2(icon.guid, guid, ARRAYSIZE(guid));
        key += L"|";
        key += guid;
    } else {
        key += L"|" + std::to_wstring(icon.id);
    }
    return key;
}

// An app added, changed or removed its icon (sent to the taskbar).
void OnTrayNotify(const COPYDATASTRUCT* copyData) {
    if (!copyData || copyData->dwData != 1 ||
        copyData->cbData < offsetof(TrayNotifyData, nid) +
                               offsetof(NotifyIconData32, szTip)) {
        return;
    }
    const auto* data = (const TrayNotifyData*)copyData->lpData;
    if (data->signature != kTrayNotifySignature) {
        return;
    }
    NotifyIconData32 nid{};
    memcpy(&nid, &data->nid,
           std::min<size_t>(copyData->cbData - offsetof(TrayNotifyData, nid),
                            sizeof(nid)));
    nid.szTip[ARRAYSIZE(nid.szTip) - 1] = L'\0';
    const bool hasGuid = (nid.uFlags & NIF_GUID) &&
                         copyData->cbData >= offsetof(TrayNotifyData, nid) +
                                                 offsetof(NotifyIconData32,
                                                          hBalloonIcon);
    const HWND owner = (HWND)(LONG_PTR)(LONG)nid.hWnd;
    // Windows' own icons (volume, network, power, microphone...), which the
    // island shows its own way: {7820AE7x-23E3-4229-82C1-E41CB67D5B9C}.
    static const BYTE kSystemIconTail[] = {0x82, 0xC1, 0xE4, 0x1C,
                                           0xB6, 0x7D, 0x5B, 0x9C};
    if (hasGuid && (nid.guidItem.Data1 & 0xFFFFFFF0) == 0x7820AE70 &&
        nid.guidItem.Data2 == 0x23E3 && nid.guidItem.Data3 == 0x4229 &&
        memcmp(nid.guidItem.Data4, kSystemIconTail, 8) == 0) {
        return;
    }

    bool changed = false;
    HWND askAgain = nullptr;
    AcquireSRWLockExclusive(&g_trayIconsLock);
    auto it = std::find_if(g_trayIcons.begin(), g_trayIcons.end(),
                           [&](const TrayIcon& icon) {
                               return hasGuid ? icon.hasGuid &&
                                                    icon.guid == nid.guidItem
                                              : !icon.hasGuid &&
                                                    icon.owner == owner &&
                                                    icon.id == nid.uID;
                           });
    switch (data->message) {
        case NIM_ADD:
        case NIM_MODIFY: {
            if (it == g_trayIcons.end()) {
                // A change to an icon added before the bridge saw it: added
                // with its picture, and its window asked once, directly, to
                // add it again (with its version, for clicks).
                if (data->message == NIM_MODIFY) {
                    if (!(nid.uFlags & NIF_ICON) || !nid.hIcon ||
                        !IsWindow(owner)) {
                        break;
                    }
                    if (std::find(g_trayOwnersAsked.begin(),
                                  g_trayOwnersAsked.end(),
                                  owner) == g_trayOwnersAsked.end()) {
                        g_trayOwnersAsked.push_back(owner);
                        askAgain = owner;
                    }
                }
                TrayIcon icon;
                icon.owner = owner;
                icon.id = nid.uID;
                icon.hasGuid = hasGuid;
                icon.guid = nid.guidItem;
                icon.version = kTrayVersionUnknown;
                GetWindowThreadProcessId(owner, &icon.processId);
                icon.key = TrayIconKey(icon);
                g_trayIcons.push_back(std::move(icon));
                it = g_trayIcons.end() - 1;
            }
            // An icon known by its GUID may come without its window
            // (H.NotifyIcon, used by WhatsApp): the one it came with stays.
            if (owner && IsWindow(owner)) {
                it->owner = owner;
                GetWindowThreadProcessId(owner, &it->processId);
            }
            if (nid.uFlags & NIF_MESSAGE) {
                it->callback = nid.uCallbackMessage;
            }
            if ((nid.uFlags & NIF_ICON) && nid.hIcon) {
                // The app may destroy its handle right after: a copy.
                if (HICON copy = CopyIcon((HICON)(LONG_PTR)(LONG)nid.hIcon)) {
                    it->icon = std::make_shared<OwnedIcon>(copy);
                    it->serial = ++g_trayIconSerial;
                }
            }
            if (nid.uFlags & NIF_TIP) {
                it->tip = nid.szTip;
            }
            if ((nid.uFlags & NIF_STATE) && (nid.dwStateMask & NIS_HIDDEN)) {
                it->hidden = (nid.dwState & NIS_HIDDEN) != 0;
            }
            changed = true;
            break;
        }
        case NIM_DELETE:
            if (it != g_trayIcons.end()) {
                g_trayIcons.erase(it);
                changed = true;
            }
            break;
        case NIM_SETVERSION:
            if (it != g_trayIcons.end()) {
                it->version = nid.uVersion;
                changed = true;
            }
            break;
    }
    // Icons whose window is gone (the app closed without removing them).
    if (std::erase_if(g_trayIcons, [](const TrayIcon& icon) {
            return !IsWindow(icon.owner);
        })) {
        changed = true;
    }
    std::erase_if(g_trayOwnersAsked,
                  [](HWND window) { return !IsWindow(window); });
    ReleaseSRWLockExclusive(&g_trayIconsLock);
    if (askAgain) {
        static const UINT taskbarCreated =
            RegisterWindowMessage(L"TaskbarCreated");
        PostMessage(askAgain, taskbarCreated, 0, 0);
    }
    // Sent a moment later, once for all the changes until then: the app
    // that changed its icon is waiting for the taskbar meanwhile.
    if (changed && !g_trayIconsSendPending.exchange(true)) {
        if (HWND taskbar = g_taskbar) {
            PostMessage(taskbar, g_sendTrayIconsMessage, 0, 0);
        } else {
            g_trayIconsSendPending = false;
        }
    }
}

// Plain values and strings, one after another.
struct IpcWriter {
    std::vector<BYTE> bytes;
    template <typename T>
    void Put(const T& value) {
        const BYTE* data = (const BYTE*)&value;
        bytes.insert(bytes.end(), data, data + sizeof(T));
    }
    void PutString(const std::wstring& text) {
        Put<UINT32>((UINT32)text.size());
        const BYTE* data = (const BYTE*)text.data();
        bytes.insert(bytes.end(), data, data + text.size() * sizeof(WCHAR));
    }
};

// To the island, in its own process: each icon's data and its icon handle
// (usable from any process of the session; the island copies it while
// handling the message, while this one is kept alive). Nothing is locked
// while sending.
void SendTrayIcons() {
    HWND island = FindWindow(kHostClassName, nullptr);
    if (!island) {
        return;
    }
    IpcWriter writer;
    std::vector<std::shared_ptr<OwnedIcon>> keptAlive;
    AcquireSRWLockShared(&g_trayIconsLock);
    writer.Put<UINT32>((UINT32)g_trayIcons.size());
    for (const auto& icon : g_trayIcons) {
        writer.Put<UINT64>((UINT64)(ULONG_PTR)icon.owner);
        writer.Put<UINT32>(icon.id);
        writer.Put<GUID>(icon.guid);
        writer.Put<BYTE>(icon.hasGuid);
        writer.Put<UINT32>(icon.callback);
        writer.Put<UINT32>(icon.version);
        writer.Put<UINT64>(
            (UINT64)(ULONG_PTR)(icon.icon ? icon.icon->icon : nullptr));
        writer.Put<UINT32>(icon.serial);
        writer.Put<BYTE>(icon.hidden);
        writer.Put<UINT32>(icon.processId);
        writer.PutString(icon.tip);
        writer.PutString(icon.key);
        keptAlive.push_back(icon.icon);
    }
    ReleaseSRWLockShared(&g_trayIconsLock);
    COPYDATASTRUCT copyData{kIpcTrayIcons, (DWORD)writer.bytes.size(),
                            writer.bytes.data()};
    DWORD_PTR result = 0;
    SendMessageTimeout(island, WM_COPYDATA, (WPARAM)(HWND)g_taskbar,
                       (LPARAM)&copyData,
                       SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT | SMTO_BLOCK,
                       kIpcTimeoutMs, &result);
}

// The first time the island asks: every app adds its icons again (the
// message Windows sends them when the taskbar restarts), so the ones added
// before the bridge loaded are known too.
void RequestTrayIcons() {
    if (g_trayIconsRequested.exchange(true)) {
        return;
    }
    static const UINT taskbarCreated = RegisterWindowMessage(L"TaskbarCreated");
    PostMessage(HWND_BROADCAST, taskbarCreated, 0, 0);
}

bool IsHostSender(HWND sender) {
    WCHAR name[64];
    return sender && GetClassName(sender, name, ARRAYSIZE(name)) &&
           wcscmp(name, kHostClassName) == 0;
}

LRESULT CALLBACK TaskbarSubclassProc(HWND hWnd,
                                     UINT uMsg,
                                     WPARAM wParam,
                                     LPARAM lParam,
                                     DWORD_PTR) {
    if (uMsg == g_sendTrayIconsMessage && g_sendTrayIconsMessage) {
        g_trayIconsSendPending = false;
        if (!g_unloading) {
            SendTrayIcons();
        }
        return 0;
    }
    if (uMsg == g_armProgressMessage && g_armProgressMessage) {
        if (!g_unloading) SetTimer(hWnd, kProgressTimer, 1000, nullptr);
        return 0;
    }
    if (uMsg == WM_TIMER && wParam == kProgressTimer) {
        if (!g_unloading) PublishProgress();
        return 0;
    }
    if (uMsg == WM_APP + 180) {
        KillTimer(hWnd, kProgressTimer);
        g_progressBars.clear();
        g_windowProgress.clear();
        return 0;
    }
    if (uMsg == WM_COPYDATA && lParam) {
        const auto* copyData = (const COPYDATASTRUCT*)lParam;
        if (copyData->dwData == kIpcHello && IsHostSender((HWND)wParam)) {
            // (Posted: the island is waiting for this answer meanwhile.)
            RequestTrayIcons();
            PostMessage(hWnd, g_sendTrayIconsMessage, 0, 0);
            return TRUE;
        }
        // The taskbar handles it first; then the bridge keeps its copy.
        const LRESULT result = DefSubclassProc(hWnd, uMsg, wParam, lParam);
        OnTrayNotify(copyData);
        return result;
    }
    return DefSubclassProc(hWnd, uMsg, wParam, lParam);
}

BOOL CALLBACK FindOwnTaskbarProc(HWND hWnd, LPARAM lParam) {
    DWORD processId = 0;
    WCHAR className[32] = L"";
    if (GetWindowThreadProcessId(hWnd, &processId) &&
        processId == GetCurrentProcessId() &&
        GetClassName(hWnd, className, ARRAYSIZE(className)) &&
        _wcsicmp(className, L"Shell_TrayWnd") == 0) {
        *(HWND*)lParam = hWnd;
        return FALSE;
    }
    return TRUE;
}

bool SubclassOwnTaskbar() {
    HWND taskbar = nullptr;
    EnumWindows(FindOwnTaskbarProc, (LPARAM)&taskbar);
    if (!taskbar || !WindhawkUtils::SetWindowSubclassFromAnyThread(
                        taskbar, TaskbarSubclassProc, 0)) {
        return false;
    }
    g_taskbar = taskbar;
    if (g_taskbarSymbols) PostMessage(taskbar, g_armProgressMessage, 0, 0);
    Wh_Log(L"Taskbar subclassed");
    // An island already running learns the bridge is here.
    if (HWND island = FindWindow(kHostClassName, nullptr)) {
        PostMessage(island, RegisterWindowMessage(L"CimoriTrayBridge_Ready"),
                    0, 0);
    }
    return true;
}

// Loaded as Explorer starts, its taskbar may not be there yet.
// ponytail: polls once a second for up to a minute; hook CreateWindowExW if
// that ever falls short.
HANDLE g_waitThread;

DWORD WINAPI WaitForTaskbarProc(LPVOID) {
    for (int i = 0; i < 60 && !g_unloading; i++) {
        Sleep(1000);
        if (SubclassOwnTaskbar()) {
            break;
        }
    }
    return 0;
}

BOOL Wh_ModInit() {
    Wh_Log(L"Init");
    g_taskbarSymbols = HookProgress();
    if (!g_taskbarSymbols) Wh_Log(L"Progress symbols unavailable: Cimori keeps BITS/partial-file fallback");
    if (!SubclassOwnTaskbar()) {
        // Not the taskbar's explorer.exe (a folder window's), or too early.
        g_waitThread =
            CreateThread(nullptr, 0, WaitForTaskbarProc, nullptr, 0, nullptr);
    }
    return TRUE;
}

void Wh_ModUninit() {
    Wh_Log(L"Uninit");
    g_unloading = true;
    if (g_waitThread) {
        WaitForSingleObject(g_waitThread, INFINITE);
        CloseHandle(g_waitThread);
        g_waitThread = nullptr;
    }
    if (HWND taskbar = g_taskbar.exchange(nullptr); taskbar && IsWindow(taskbar)) {
        // Stop the heartbeat and revoke WinRT callbacks on their owning
        // taskbar thread before unloading.
        SendMessage(taskbar, WM_APP + 180, 0, 0);
        WindhawkUtils::RemoveWindowSubclassFromAnyThread(taskbar,
                                                         TaskbarSubclassProc);
    }
    AcquireSRWLockExclusive(&g_trayIconsLock);
    g_trayIcons.clear();
    ReleaseSRWLockExclusive(&g_trayIconsLock);
}
