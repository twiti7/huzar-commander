; Inno Setup Script for Huzar Commander
; Dual-panel orthodox file manager in Rust (eframe/egui)

#define MyAppName "Huzar Commander"
#ifndef MyAppVersion
  #define MyAppVersion "0.3.0"
#endif
#define MyAppPublisher "Huzar Commander Team"
#define MyAppURL "https://github.com/twiti7/huzar-commander"
#define MyAppExeName "huzar-commander.exe"

[Setup]
AppId={{8B2C4E3D-7F1A-4D90-A8C5-3E9B6A1E2F4C}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=..\target\installer
OutputBaseFilename=HuzarCommander-Setup-v{#MyAppVersion}
SetupIconFile=..\assets\huzar_icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "polish"; MessagesFile: "compiler:Languages\Polish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
polish.DesktopShortcut=Utwórz skrót na Pulpicie
english.DesktopShortcut=Create a desktop shortcut
polish.ContextMenu=Dodaj "Otwórz w programie Huzar Commander" do menu kontekstowego folderów
english.ContextMenu=Add "Open in Huzar Commander" to folder context menu
polish.AddToPath=Dodaj do zmiennej środowiskowej PATH (polecenie huzar-commander)
english.AddToPath=Add to system PATH environment variable (huzar-commander command)
polish.RunNow=Uruchom Huzar Commander po zakończeniu instalacji
english.RunNow=Launch Huzar Commander after installation

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopShortcut}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "contextmenu"; Description: "{cm:ContextMenu}"; Flags: checkedonce
Name: "addtopath"; Description: "{cm:AddToPath}"; Flags: unchecked

[Files]
; Główny plik wykonywalny
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
; Ikona i zasoby
Source: "..\assets\huzar_icon.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\huzar_icon.png"; DestDir: "{app}"; Flags: ignoreversion
; Pliki językowe (zewnętrzne kopie)
Source: "..\locales\*"; DestDir: "{app}\locales"; Flags: ignoreversion recursesubdirs createallsubdirs
; Dokumentacja
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "..\README_PL.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\huzar_icon.ico"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\huzar_icon.ico"; Tasks: desktopicon

[Registry]
; Menu kontekstowe Eksploratora Windows dla folderów
Root: HKA; Subkey: "Software\Classes\Directory\shell\HuzarCommander"; ValueType: string; ValueName: ""; ValueData: "Otwórz w programie Huzar Commander"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\shell\HuzarCommander"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\huzar_icon.ico"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\HuzarCommander\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: contextmenu

; Menu kontekstowe w tle folderu (puste miejsce w otwartym oknie Eksploratora)
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\HuzarCommander"; ValueType: string; ValueName: ""; ValueData: "Otwórz w programie Huzar Commander"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\HuzarCommander"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\huzar_icon.ico"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\HuzarCommander\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%V"""; Tasks: contextmenu

; Menu kontekstowe dla dysków (C:, D: itp.)
Root: HKA; Subkey: "Software\Classes\Drive\shell\HuzarCommander"; ValueType: string; ValueName: ""; ValueData: "Otwórz w programie Huzar Commander"; Tasks: contextmenu; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Drive\shell\HuzarCommander"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\huzar_icon.ico"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Drive\shell\HuzarCommander\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: contextmenu

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:RunNow}"; Flags: nowait postinstall skipifsilent

[Code]
// Opcjonalne dodanie do PATH
procedure CurStepChanged(CurStep: TSetupStep);
var
  PathStr: string;
  AppDir: string;
  RootKey: Integer;
  SubKey: string;
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
  begin
    AppDir := ExpandConstant('{app}');
    if IsAdminInstallMode then
    begin
      RootKey := HKEY_LOCAL_MACHINE;
      SubKey := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';
    end
    else
    begin
      RootKey := HKEY_CURRENT_USER;
      SubKey := 'Environment';
    end;

    if RegQueryStringValue(RootKey, SubKey, 'Path', PathStr) then
    begin
      if Pos(AppDir, PathStr) = 0 then
      begin
        PathStr := PathStr + ';' + AppDir;
        RegWriteStringValue(RootKey, SubKey, 'Path', PathStr);
      end;
    end;
  end;
end;
