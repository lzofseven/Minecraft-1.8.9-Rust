; Instalador Windows do mine-rs (NSIS 3). Gerado por scripts/build-windows.sh:
;   makensis -DVERSAO=<versão> -DEXE=<mc-client.exe> -DSAIDA=<instalador.exe> windows/instalador.nsi
; Instala só para o usuário (sem admin) em %LOCALAPPDATA%\Minecraft 1.8.9 Rust.

; 64 bits, como o jogo (num Windows 32 bits o instalador nem abre).
Target amd64-unicode
!include "MUI2.nsh"

!define NOME "Minecraft 1.8.9 Rust"
!define CHAVE "Software\Microsoft\Windows\CurrentVersion\Uninstall\MineRs189"
!ifndef VERSAO
  !define VERSAO "0.1.0"
!endif

Name "${NOME}"
OutFile "${SAIDA}"
InstallDir "$LOCALAPPDATA\${NOME}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "${NOME} ${VERSAO}"

!define MUI_ICON "icone.ico"
!define MUI_UNICON "icone.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "lateral.bmp"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\mine-rs.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Abrir o jogo agora"

!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "PortugueseBR"

VIProductVersion "${VERSAO}.0"
VIAddVersionKey /LANG=${LANG_PORTUGUESEBR} "ProductName" "${NOME}"
VIAddVersionKey /LANG=${LANG_PORTUGUESEBR} "FileDescription" "Instalador do ${NOME}"
VIAddVersionKey /LANG=${LANG_PORTUGUESEBR} "FileVersion" "${VERSAO}"
VIAddVersionKey /LANG=${LANG_PORTUGUESEBR} "ProductVersion" "${VERSAO}"
VIAddVersionKey /LANG=${LANG_PORTUGUESEBR} "LegalCopyright" "GPL-3.0"

Section "Jogo"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File "/oname=mine-rs.exe" "${EXE}"
  File "icone.ico"
  WriteUninstaller "$INSTDIR\desinstalar.exe"

  CreateShortcut "$DESKTOP\${NOME}.lnk" "$INSTDIR\mine-rs.exe" "" "$INSTDIR\icone.ico" 0
  CreateShortcut "$SMPROGRAMS\${NOME}.lnk" "$INSTDIR\mine-rs.exe" "" "$INSTDIR\icone.ico" 0

  WriteRegStr HKCU "${CHAVE}" "DisplayName" "${NOME}"
  WriteRegStr HKCU "${CHAVE}" "DisplayVersion" "${VERSAO}"
  WriteRegStr HKCU "${CHAVE}" "Publisher" "lzofseven"
  WriteRegStr HKCU "${CHAVE}" "DisplayIcon" "$INSTDIR\icone.ico"
  WriteRegStr HKCU "${CHAVE}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${CHAVE}" "UninstallString" '"$INSTDIR\desinstalar.exe"'
  WriteRegStr HKCU "${CHAVE}" "QuietUninstallString" '"$INSTDIR\desinstalar.exe" /S'
  WriteRegDWORD HKCU "${CHAVE}" "NoModify" 1
  WriteRegDWORD HKCU "${CHAVE}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$DESKTOP\${NOME}.lnk"
  Delete "$SMPROGRAMS\${NOME}.lnk"
  Delete "$INSTDIR\mine-rs.exe"
  Delete "$INSTDIR\icone.ico"
  Delete "$INSTDIR\desinstalar.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${CHAVE}"
  ; Os arquivos baixados (jar, sons, terreno guardado) e as configurações ficam, salvo pedido.
  MessageBox MB_YESNO|MB_DEFBUTTON2 "Apagar também as configurações, a conta salva e os arquivos baixados do jogo?" /SD IDNO IDNO fim
    RMDir /r "$LOCALAPPDATA\mine-rs"
    RMDir /r "$APPDATA\mine-rs"
  fim:
SectionEnd
