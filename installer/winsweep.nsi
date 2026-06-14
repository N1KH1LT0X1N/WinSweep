; WinSweep Installer Script (NSIS)
; Build with: makensis installer\winsweep.nsi

!include "MUI2.nsh"
!include "LogicLib.nsh"

!define PRODUCT_NAME "WinSweep"
!ifndef PRODUCT_VERSION
  !define PRODUCT_VERSION "0.1.0"
!endif
!define PRODUCT_PUBLISHER "WinSweep Team"
!define PRODUCT_WEB_SITE "https://github.com/winsweep/winsweep"
!define PRODUCT_DIR_REGKEY "Software\Microsoft\Windows\CurrentVersion\App Paths\winsweep-gui.exe"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}"
!define PRODUCT_UNINST_ROOT_KEY "HKLM"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "winsweep-${PRODUCT_VERSION}-setup.exe"
InstallDir "$PROGRAMFILES64\WinSweep"
InstallDirRegKey HKLM "${PRODUCT_DIR_REGKEY}" ""
ShowInstDetails show
ShowUnInstDetails show
RequestExecutionLevel admin

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_INSTFILES

; Languages
!insertmacro MUI_LANGUAGE "English"

; Sections
Section "Core Files" SEC_CORE
  SetOutPath "$INSTDIR"
  File "..\target\x86_64-pc-windows-gnu\release\winsweep-gui.exe"
  File "..\target\x86_64-pc-windows-gnu\release\winsweep-cli.exe"
  File "..\README.md"

  ; Start Menu shortcuts
  CreateDirectory "$SMPROGRAMS\WinSweep"
  CreateShortcut "$SMPROGRAMS\WinSweep\WinSweep.lnk" "$INSTDIR\winsweep-gui.exe"
  CreateShortcut "$SMPROGRAMS\WinSweep\Uninstall.lnk" "$INSTDIR\uninstall.exe"

  ; Uninstall registry entries
  WriteRegStr HKLM "${PRODUCT_DIR_REGKEY}" "" "$INSTDIR\winsweep-gui.exe"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\winsweep-gui.exe"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
  WriteRegDWORD ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "NoModify" 1
  WriteRegDWORD ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}" "NoRepair" 1
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Add CLI to PATH" SEC_PATH
  ; Append to system PATH
  ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
  StrCmp $0 "" skip_path
  Push $0
  Push "$INSTDIR"
  Call AddToPath
skip_path:
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\winsweep-gui.exe"
  Delete "$INSTDIR\winsweep-cli.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\uninstall.exe"

  Delete "$SMPROGRAMS\WinSweep\WinSweep.lnk"
  Delete "$SMPROGRAMS\WinSweep\Uninstall.lnk"
  RMDir "$SMPROGRAMS\WinSweep"
  RMDir "$INSTDIR"

  DeleteRegKey ${PRODUCT_UNINST_ROOT_KEY} "${PRODUCT_UNINST_KEY}"
  DeleteRegKey HKLM "${PRODUCT_DIR_REGKEY}"
SectionEnd

; Function: AddToPath (appends a directory to the system PATH)
Function AddToPath
  Exch $R0
  Push $R1
  Push $R2
  Push $R3

  ReadRegStr $R1 HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path"
  StrCmp $R1 "" done

  ; Check if already present
  Push $R1
  Push $R0
  Call StrStr
  Pop $R2
  StrCmp $R2 "" 0 done

  StrCpy $R2 "$R1;$R0"
  WriteRegExpandStr HKLM "SYSTEM\CurrentControlSet\Control\Session Manager\Environment" "Path" "$R2"
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000

done:
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
FunctionEnd

; Function: StrStr (finds a substring)
Function StrStr
  Exch $R1
  Exch 1
  Exch $R0
  Push $R2
  Push $R3
  Push $R4
  StrCpy $R3 -1
  StrLen $R4 $R1
  IntOp $R4 $R4 - 1
  StrCpy $R2 0
loop:
  IntOp $R2 $R2 + 1
  StrCpy $R3 $R1 $R4 $R2
  StrCmp $R3 $R1 done
  StrCmp $R3 "" done
  Goto loop
done:
  StrCpy $R1 $R2
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R0
  Exch $R1
FunctionEnd
