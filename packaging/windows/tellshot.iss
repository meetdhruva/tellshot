; Tellshot installer (Inno Setup 6). Built by .github/workflows/release.yml:
;   iscc /DAppVersion=0.1.0 /DArch=x64 /DSourceDir=<staging> /DOutputDir=<dist> tellshot.iss
; Arch is x64 or arm64. Installs per user by default (no admin prompt); an admin can choose
; "install for all users" in the first dialog.

#ifndef AppVersion
  #error AppVersion must be defined
#endif
#ifndef Arch
  #error Arch must be defined (x64 or arm64)
#endif

[Setup]
AppId={{6F3B0C2E-9D3A-4E3B-9B1E-7C2A5D9F4E10}
AppName=Tellshot
AppVersion={#AppVersion}
AppVerName=Tellshot {#AppVersion}
AppPublisher=Tellshot contributors
AppPublisherURL=https://github.com/meetdhruva/tellshot
AppSupportURL=https://github.com/meetdhruva/tellshot/issues
AppUpdatesURL=https://github.com/meetdhruva/tellshot/releases
DefaultDirName={autopf}\Tellshot
DefaultGroupName=Tellshot
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
MinVersion=10.0.17763
LicenseFile={#SourceDir}\LICENSE-MIT
SetupIconFile={#SourceDir}\tellshot.ico
UninstallDisplayIcon={app}\tellshot-editor.exe
OutputDir={#OutputDir}
OutputBaseFilename=tellshot-{#AppVersion}-{#Arch}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes
ChangesEnvironment=yes
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "addtopath"; Description: "Add the tellshot command to PATH"; GroupDescription: "Command line:"
Name: "associate"; Description: "Open .tellshot projects with Tellshot"; GroupDescription: "File types:"

[Files]
Source: "{#SourceDir}\tellshot-editor.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\tellshot.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "{#SourceDir}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Tellshot"; Filename: "{app}\tellshot-editor.exe"
Name: "{group}\Uninstall Tellshot"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Tellshot"; Filename: "{app}\tellshot-editor.exe"; Tasks: desktopicon

[Registry]
; File association (per user or machine, following the install mode).
Root: HKA; Subkey: "Software\Classes\.tellshot"; ValueType: string; ValueName: ""; ValueData: "Tellshot.Project"; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.tellshot\OpenWithProgids"; ValueType: string; ValueName: "Tellshot.Project"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Tellshot.Project"; ValueType: string; ValueName: ""; ValueData: "Tellshot project"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Tellshot.Project\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\tellshot-editor.exe,0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Tellshot.Project\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\tellshot-editor.exe"" ""%1"""; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Applications\tellshot-editor.exe\SupportedTypes"; ValueType: string; ValueName: ".tellshot"; ValueData: ""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\tellshot-editor.exe\SupportedTypes"; ValueType: string; ValueName: ".png"; ValueData: ""

[Run]
Filename: "{app}\tellshot-editor.exe"; Description: "{cm:LaunchProgram,Tellshot}"; Flags: nowait postinstall skipifsilent

[Code]
// Adds or removes {app} on the user's (or, for admin installs, the system's) PATH.
function PathKey(): String;
begin
  if IsAdminInstallMode() then
    Result := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
  else
    Result := 'Environment';
end;

function PathRoot(): Integer;
begin
  if IsAdminInstallMode() then
    Result := HKEY_LOCAL_MACHINE
  else
    Result := HKEY_CURRENT_USER;
end;

procedure EditPath(Dir: String; Add: Boolean);
var
  Paths: String;
  Needle: String;
  P: Integer;
begin
  if not RegQueryStringValue(PathRoot(), PathKey(), 'Path', Paths) then
    Paths := '';
  Needle := ';' + Uppercase(Dir) + ';';
  P := Pos(Needle, ';' + Uppercase(Paths) + ';');
  if Add and (P = 0) then
  begin
    if (Paths <> '') and (Copy(Paths, Length(Paths), 1) <> ';') then
      Paths := Paths + ';';
    RegWriteExpandStringValue(PathRoot(), PathKey(), 'Path', Paths + Dir);
  end
  else if (not Add) and (P > 0) then
  begin
    Delete(Paths, P, Length(Dir) + 1);
    if Copy(Paths, Length(Paths), 1) = ';' then
      Delete(Paths, Length(Paths), 1);
    RegWriteExpandStringValue(PathRoot(), PathKey(), 'Path', Paths);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    EditPath(ExpandConstant('{app}'), True);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    EditPath(ExpandConstant('{app}'), False);
end;
