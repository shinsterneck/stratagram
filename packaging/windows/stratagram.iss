; This file is part of Stratagram: https://github.com/shinsterneck/stratagram
; Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
; SPDX-License-Identifier: Apache-2.0

; The Windows installer (Inno Setup 6), made from the bundle that the Build
; workflow puts together: stratagram.exe with Qt next to it (windeployqt),
; the licenses and README-Qt.txt. From the repository's root:
;   iscc /DVersion=1.0.0 /DBundle=..\..\dist\stratagram packaging\windows\stratagram.iss
; writes dist\stratagram-1.0.0-windows-x64-setup.exe.
;
; It installs for the current user or for all users (asked at the start, or
; /CURRENTUSER and /ALLUSERS), adds a Start menu entry and opens .stratagram
; files with Stratagram. /VERYSILENT installs without questions.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Bundle
  #define Bundle "..\..\dist\stratagram"
#endif

[Setup]
; The same for every version, so a new one replaces the old one in place.
AppId={{05B3C096-C2A0-421F-8ECD-3F21F1C18E27}
AppName=Stratagram
AppVersion={#Version}
AppVerName=Stratagram {#Version}
AppPublisher=Shin Sterneck
AppPublisherURL=https://stratagram.app/
AppSupportURL=https://github.com/shinsterneck/stratagram/issues
AppUpdatesURL=https://github.com/shinsterneck/stratagram/releases
AppCopyright=Copyright 2026 Shin Sterneck
VersionInfoVersion={#Version}
DefaultDirName={autopf}\Stratagram
DefaultGroupName=Stratagram
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesAssociations=yes
CloseApplications=yes
SetupIconFile=stratagram.ico
UninstallDisplayIcon={app}\stratagram.exe
UninstallDisplayName=Stratagram
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir=..\..\dist
OutputBaseFilename=stratagram-{#Version}-windows-x64-setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "assoc"; Description: "Open .stratagram files with Stratagram"; GroupDescription: "File types:"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Bundle}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Stratagram"; Filename: "{app}\stratagram.exe"
Name: "{autodesktop}\Stratagram"; Filename: "{app}\stratagram.exe"; Tasks: desktopicon

[Registry]
; .stratagram opens with Stratagram, with the app's icon, and Stratagram
; is offered in "Open with". If someone has picked another program for
; .stratagram in Windows, that choice stays. Taken out again when
; uninstalled.
Root: HKA; Subkey: "Software\Classes\.stratagram"; ValueType: string; ValueName: ""; ValueData: "Stratagram.Diagram"; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.stratagram\OpenWithProgids"; ValueType: string; ValueName: "Stratagram.Diagram"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Stratagram.Diagram"; ValueType: string; ValueName: ""; ValueData: "Stratagram diagram"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Stratagram.Diagram\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\stratagram.exe,0"; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Stratagram.Diagram\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\stratagram.exe"" ""%1"""; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Applications\stratagram.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Stratagram"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\stratagram.exe\SupportedTypes"; ValueType: string; ValueName: ".stratagram"; ValueData: ""

[Run]
Filename: "{app}\stratagram.exe"; Description: "{cm:LaunchProgram,Stratagram}"; Flags: nowait postinstall skipifsilent
