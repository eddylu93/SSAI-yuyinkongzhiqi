Unicode true
RequestExecutionLevel user
Name "SSAI-语音控制器"
!define STAGE "..\release\windows-0.8.1"
OutFile "${STAGE}\SSAI-yuyinkongzhiqi-0.8.1-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\AU05 Desk"
!include "MUI2.nsh"
!define MUI_ICON "..\src-tauri\icons\icon.ico"
!define MUI_UNICON "..\src-tauri\icons\icon.ico"
!define APPKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\AU05Desk"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
Section "SSAI-语音控制器"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File "${STAGE}\AU05Desk.exe"
  File "${STAGE}\WebView2Loader.dll"
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
  WriteRegStr HKCU "${APPKEY}" "DisplayName" "SSAI-语音控制器"
  WriteRegStr HKCU "${APPKEY}" "DisplayIcon" '"$INSTDIR\AU05Desk.exe",0'
  WriteRegStr HKCU "${APPKEY}" "Publisher" "三笙"
  WriteRegStr HKCU "${APPKEY}" "URLInfoAbout" "https://github.com/eddylu93/SSAI-yuyinkongzhiqi"
  WriteRegStr HKCU "${APPKEY}" "DisplayVersion" "0.8.1"
  WriteRegStr HKCU "${APPKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
SectionEnd
Section "Uninstall"
  SetShellVarContext current
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "AU05Desk"
  Delete "$SMPROGRAMS\AU05 Desk.lnk"
  Delete "$SMPROGRAMS\SSAI-语音控制器.lnk"
  Delete "$INSTDIR\AU05Desk.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\AU05-Desk-source.zip"
  Delete "$INSTDIR\ThirdPartyNotices.txt"
  Delete "$INSTDIR\libwinpthread-1.dll"
  Delete "$INSTDIR\libgcc_s_seh-1.dll"
  Delete "$INSTDIR\libstdc++-6.dll"
  Delete "$INSTDIR\WebView2Loader.dll"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${APPKEY}"
SectionEnd
