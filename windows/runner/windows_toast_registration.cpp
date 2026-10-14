#include "windows_toast_registration.h"

#include <propkey.h>
#include <propvarutil.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <wrl/client.h>

#include <string>

namespace {

constexpr wchar_t kAppUserModelId[] = L"OpenChat.Desktop";
constexpr wchar_t kShortcutName[] = L"OpenChat.lnk";

}  // namespace

HRESULT EnsureOpenChatToastRegistration() {
  const HRESULT app_id_result =
      SetCurrentProcessExplicitAppUserModelID(kAppUserModelId);
  if (FAILED(app_id_result)) {
    return app_id_result;
  }

  PWSTR programs_path = nullptr;
  const HRESULT folder_result =
      SHGetKnownFolderPath(FOLDERID_Programs, KF_FLAG_CREATE, nullptr,
                           &programs_path);
  if (FAILED(folder_result)) {
    return folder_result;
  }

  std::wstring shortcut_path(programs_path);
  CoTaskMemFree(programs_path);
  shortcut_path.append(L"\\").append(kShortcutName);

  std::wstring executable_path(32768, L'\0');
  const DWORD executable_length = GetModuleFileNameW(
      nullptr, executable_path.data(),
      static_cast<DWORD>(executable_path.size()));
  if (executable_length == 0) {
    const DWORD error = GetLastError();
    return HRESULT_FROM_WIN32(
        error == ERROR_SUCCESS ? ERROR_GEN_FAILURE : error);
  }
  if (executable_length >= executable_path.size()) {
    return HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER);
  }
  executable_path.resize(executable_length);

  Microsoft::WRL::ComPtr<IShellLinkW> shell_link;
  HRESULT result = CoCreateInstance(CLSID_ShellLink, nullptr, CLSCTX_INPROC_SERVER,
                                    IID_PPV_ARGS(&shell_link));
  if (FAILED(result)) {
    return result;
  }

  result = shell_link->SetPath(executable_path.c_str());
  if (FAILED(result)) {
    return result;
  }
  result = shell_link->SetDescription(L"OpenChat");
  if (FAILED(result)) {
    return result;
  }
  result = shell_link->SetIconLocation(executable_path.c_str(), 0);
  if (FAILED(result)) {
    return result;
  }

  Microsoft::WRL::ComPtr<IPropertyStore> property_store;
  result = shell_link.As(&property_store);
  if (FAILED(result)) {
    return result;
  }

  PROPVARIANT app_id_value;
  PropVariantInit(&app_id_value);
  result = InitPropVariantFromString(kAppUserModelId, &app_id_value);
  if (SUCCEEDED(result)) {
    result = property_store->SetValue(PKEY_AppUserModel_ID, app_id_value);
  }
  PropVariantClear(&app_id_value);
  if (FAILED(result)) {
    return result;
  }

  result = property_store->Commit();
  if (FAILED(result)) {
    return result;
  }

  Microsoft::WRL::ComPtr<IPersistFile> persist_file;
  result = shell_link.As(&persist_file);
  if (FAILED(result)) {
    return result;
  }
  return persist_file->Save(shortcut_path.c_str(), TRUE);
}
