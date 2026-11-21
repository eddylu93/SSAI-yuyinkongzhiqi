Unicode true
RequestExecutionLevel user
Name "SSAI-语音控制器"
!define STAGE "..\release\windows-0.8.3"
OutFile "${STAGE}\SSAI-yuyinkongzhiqi-0.8.3-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\AU05 Desk"
!include "MUI2.nsh"
!include "LogicLib.nsh"
!define WEBVIEW_KEY "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
!define MUI_ICON "..\src-tauri\icons\icon.ico"
!define MUI_UNICON "..\src-tauri\icons\icon.ico"
!define APPKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\AU05Desk"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
Function CheckWebView
  SetRegView 32
  ReadRegStr $0 HKLM "${WEBVIEW_KEY}" "pv"
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKCU "${WEBVIEW_KEY}" "pv"
  ${EndIf}
  SetRegView lastused
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    MessageBox MB_OKCANCEL|MB_ICONINFORMATION "运行软件需要 Microsoft Edge WebView2。点击“确定”打开微软官方下载页。安装 Evergreen Runtime 后，请重新运行本安装包。" /SD IDCANCEL IDCANCEL stop
    ExecShell "open" "https://developer.microsoft.com/microsoft-edge/webview2/#download-section"
    stop:
    SetErrorLevel 3
    Abort
  ${EndIf}
FunctionEnd

Function CloseRunningController
  IfFileExists "$INSTDIR\AU05Desk.exe" 0 ready
  InitPluginsDir
  File "/oname=$PLUGINSDIR\RunningController.exe" "${STAGE}\RunningController.exe"
  check:
  ClearErrors
  ExecWait '"$PLUGINSDIR\RunningController.exe" check "$INSTDIR\AU05Desk.exe"' $0
  IfErrors retry
  StrCmp $0 0 ready
  StrCmp $0 1 0 retry
  MessageBox MB_OKCANCEL|MB_ICONQUESTION "SSAI-语音控制器正在运行。点击“确定”退出软件并继续安装，点击“取消”保留当前版本。" /SD IDCANCEL IDCANCEL stop
  ClearErrors
  ExecWait '"$PLUGINSDIR\RunningController.exe" close "$INSTDIR\AU05Desk.exe"' $0
  IfErrors retry
  StrCmp $0 0 ready
  retry:
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "无法确认旧版已退出。请在系统托盘退出 SSAI-语音控制器，然后点击“重试”。" /SD IDCANCEL IDRETRY check
  stop:
  SetErrorLevel 2
  Abort
  ready:
FunctionEnd

Section "SSAI-语音控制器"
  SetShellVarContext current
  Call CheckWebView
  Call CloseRunningController
  SetOutPath "$INSTDIR"
  File "${STAGE}\AU05Desk.exe"
  File "${STAGE}\WebView2Loader.dll"
  File "${STAGE}\WeChatVoiceFocus.exe"
  !if /FileExists "${STAGE}\lib*.dll"
    File "${STAGE}\lib*.dll"
  !endif
  File "${STAGE}\AU05-Desk-source.zip"
  File "${STAGE}\ThirdPartyNotices.txt"
  File "..\LICENSE"
  File "..\README.md"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\AU05 Desk.lnk"
  CreateShortcut "$SMPROGRAMS\SSAI-语音控制器.lnk" "$INSTDIR\AU05Desk.exe"
  CreateShortcut "$DESKTOP\SSAI-语音控制器.lnk" "$INSTDIR\AU05Desk.exe" "" "$INSTDIR\AU05Desk.exe" 0
  WriteRegStr HKCU "${APPKEY}" "DisplayName" "SSAI-语音控制器"
  WriteRegStr HKCU "${APPKEY}" "DisplayIcon" '"$INSTDIR\AU05Desk.exe",0'
  WriteRegStr HKCU "${APPKEY}" "Publisher" "三笙"
  WriteRegStr HKCU "${APPKEY}" "URLInfoAbout" "https://github.com/eddylu93/SSAI-yuyinkongzhiqi"
  WriteRegStr HKCU "${APPKEY}" "DisplayVersion" "0.8.3"
  WriteRegStr HKCU "${APPKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
SectionEnd
Section "Uninstall"
  SetShellVarContext current
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "AU05Desk"
  Delete "$SMPROGRAMS\AU05 Desk.lnk"
  Delete "$SMPROGRAMS\SSAI-语音控制器.lnk"
  Delete "$DESKTOP\SSAI-语音控制器.lnk"
  Delete "$INSTDIR\AU05Desk.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\AU05-Desk-source.zip"
  Delete "$INSTDIR\ThirdPartyNotices.txt"
  Delete "$INSTDIR\libwinpthread-1.dll"
  Delete "$INSTDIR\libgcc_s_seh-1.dll"
  Delete "$INSTDIR\libstdc++-6.dll"
  Delete "$INSTDIR\WebView2Loader.dll"
  Delete "$INSTDIR\WeChatVoiceFocus.exe"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${APPKEY}"
SectionEnd
