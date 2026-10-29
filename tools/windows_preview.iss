; Windows 1.1.0.10 candidate installer: isolate from the formal desktop and sync service.
#define TenRateVersion "1.1.0.10"
[Setup]
AppId={{D14BB1FC-A178-4C36-B787-4E8DFE9089B8}
AppName=TenRate Windows Preview
AppVersion={#TenRateVersion}
AppVerName=TenRate Windows {#TenRateVersion} Preview
AppPublisher=TenRate
DefaultDirName={localappdata}\Programs\TenRatePreview
DefaultGroupName=TenRate Windows Preview
DisableProgramGroupPage=yes
DisableWelcomePage=no
PrivilegesRequired=lowest
UsePreviousAppDir=no
Uninstallable=yes
CloseApplications=no
RestartApplications=no
OutputBaseFilename=TenRate_Windows_v{#TenRateVersion}_preview_setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SourceDir=..
[Files]
Source: "native\gridtimer_native\target\release\timer_windows_client.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "tools\windows_preview_README.txt"; DestDir: "{app}"; DestName: "README.txt"; Flags: ignoreversion
[Tasks]
Name: "desktopicon"; Description: "Create a separate preview desktop shortcut"; GroupDescription: "Additional shortcuts:"
[Icons]
Name: "{group}\TenRate Windows Preview"; Filename: "{app}\timer_windows_client.exe"
Name: "{autodesktop}\TenRate Windows Preview"; Filename: "{app}\timer_windows_client.exe"; Tasks: desktopicon
