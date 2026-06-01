program OxideChk;

uses
  Crt;

const
  Unknown = 'unknown';
  DropDorInfo = 'DORINFO1.DEF';
  DropDoorSys = 'DOOR.SYS';
  NodeFile = 'OXNODE.TXT';
  ReportFile = 'OXIDECHK.RPT';

type
  DropLines = array[1..10] of string;

var
  ActiveDropFile: string;
  BoardName: string;
  SysopName: string;
  CallerName: string;
  CallerAlias: string;
  CallerLocation: string;
  SecurityLevel: string;
  MinutesRemaining: string;
  NodeNumber: string;
  Choice: char;

function ValueOrUnknown(Value: string): string;
begin
  if Value = '' then
    ValueOrUnknown := Unknown
  else
    ValueOrUnknown := Value;
end;

function FileExists(Name: string): boolean;
var
  F: file;
  Exists: boolean;
begin
  Assign(F, Name);
  {$I-}
  Reset(F, 1);
  {$I+}
  Exists := IOResult = 0;
  FileExists := Exists;
  if Exists then
    Close(F);
end;

procedure InitSummary;
begin
  ActiveDropFile := Unknown;
  BoardName := Unknown;
  SysopName := Unknown;
  CallerName := Unknown;
  CallerAlias := Unknown;
  CallerLocation := Unknown;
  SecurityLevel := Unknown;
  MinutesRemaining := Unknown;
  NodeNumber := Unknown;
end;

procedure ReadLines(Name: string; var Lines: DropLines);
var
  F: Text;
  I: integer;
begin
  for I := 1 to 10 do
    Lines[I] := '';

  Assign(F, Name);
  {$I-}
  Reset(F);
  {$I+}
  if IOResult <> 0 then
    Exit;

  I := 1;
  while (not Eof(F)) and (I <= 10) do
  begin
    ReadLn(F, Lines[I]);
    I := I + 1;
  end;
  Close(F);
end;

procedure ReadNodeFile;
var
  F: Text;
  Line: string;
begin
  if not FileExists(NodeFile) then
    Exit;

  Assign(F, NodeFile);
  {$I-}
  Reset(F);
  {$I+}
  if IOResult <> 0 then
    Exit;

  if not Eof(F) then
  begin
    ReadLn(F, Line);
    if Copy(Line, 1, 5) = 'node=' then
      NodeNumber := ValueOrUnknown(Copy(Line, 6, Length(Line) - 5));
  end;
  Close(F);
end;

procedure ParseDorInfo;
var
  Lines: DropLines;
begin
  ReadLines(DropDorInfo, Lines);
  ActiveDropFile := DropDorInfo;
  BoardName := ValueOrUnknown(Lines[1]);
  SysopName := ValueOrUnknown(Lines[2]);
  CallerName := ValueOrUnknown(Lines[6] + ' ' + Lines[7]);
  CallerAlias := CallerName;
  CallerLocation := ValueOrUnknown(Lines[8]);
  SecurityLevel := ValueOrUnknown(Lines[9]);
  MinutesRemaining := ValueOrUnknown(Lines[10]);
end;

procedure ParseDoorSys;
var
  Lines: DropLines;
begin
  ReadLines(DropDoorSys, Lines);
  ActiveDropFile := DropDoorSys;
  NodeNumber := ValueOrUnknown(Lines[4]);
  MinutesRemaining := ValueOrUnknown(Lines[5]);
  CallerAlias := ValueOrUnknown(Lines[6]);
  CallerName := ValueOrUnknown(Lines[7]);
  CallerLocation := ValueOrUnknown(Lines[8]);
  SecurityLevel := ValueOrUnknown(Lines[9]);
end;

function LoadDropFile: boolean;
begin
  LoadDropFile := true;
  if FileExists(DropDorInfo) then
    ParseDorInfo
  else if FileExists(DropDoorSys) then
    ParseDoorSys
  else
    LoadDropFile := false;

  if LoadDropFile then
    ReadNodeFile;
end;

procedure PrintSummary;
begin
  WriteLn;
  WriteLn('Drop file: ', ActiveDropFile);
  WriteLn('Node: ', NodeNumber);
  if BoardName <> Unknown then
    WriteLn('Board: ', BoardName);
  if SysopName <> Unknown then
    WriteLn('Sysop: ', SysopName);
  WriteLn('Caller: ', CallerName);
  if CallerAlias <> CallerName then
    WriteLn('Alias: ', CallerAlias);
  WriteLn('Location: ', CallerLocation);
  WriteLn('Security: ', SecurityLevel);
  WriteLn('Minutes: ', MinutesRemaining);
  WriteLn;
end;

procedure WriteReport;
var
  F: Text;
begin
  Assign(F, ReportFile);
  {$I-}
  Rewrite(F);
  {$I+}
  if IOResult <> 0 then
  begin
    WriteLn('ERROR: report file write failed');
    Halt(3);
  end;

  WriteLn(F, 'Oxide Door Check');
  WriteLn(F, 'drop_file=', ActiveDropFile);
  WriteLn(F, 'node=', NodeNumber);
  WriteLn(F, 'caller=', CallerName);
  WriteLn(F, 'result=report');
  Close(F);
  WriteLn('Report file written');
end;

procedure Prompt;
begin
  Write('[I]nfo  [R]eport  [Q]uit: ');
end;

begin
  InitSummary;
  WriteLn('Oxide Door Check');

  if not LoadDropFile then
  begin
    WriteLn('ERROR: no supported drop file found');
    Halt(2);
  end;

  PrintSummary;

  repeat
    Prompt;
    Choice := UpCase(ReadKey);
    WriteLn(Choice);
    case Choice of
      'I': PrintSummary;
      'R': WriteReport;
      'Q':
        begin
          WriteLn('Returning to OxideBBS');
          Halt(0);
        end;
    end;
  until false;
end.
