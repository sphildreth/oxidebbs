program OxideChk;

const
  Unknown = 'unknown';
  DropDorInfo = 'DORINFO1.DEF';
  DropDoorSys = 'DOOR.SYS';
  NodeFile = 'OXNODE.TXT';
  ReportFile = 'OXIDECHK.RPT';
  Com1Base = $3F8;
  Com1Data = Com1Base;
  Com1InterruptEnable = Com1Base + 1;
  Com1FifoControl = Com1Base + 2;
  Com1LineControl = Com1Base + 3;
  Com1ModemControl = Com1Base + 4;
  Com1LineStatus = Com1Base + 5;
  Com1LineStatusDataReady = $01;
  Com1LineStatusTransmitterReady = $20;
  Com1LineControlDlab = $80;
  Com1LineControl8N1 = $03;
  Com1ModemControlReady = $0B;
  Com1FifoEnableClear = $C7;
  BaudDivisor38400 = 3;

type
  DropLine = string[120];
  DropLines = array[1..10] of DropLine;
  FileBuffer = array[1..256] of Byte;

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

function UpperAscii(const Value: Char): Char;
var
  Code: Byte;
begin
  Code := Ord(Value);
  if (Code >= Ord('a')) and (Code <= Ord('z')) then
    Code := Code - 32;
  UpperAscii := Char(Code);
end;

function ValueOrUnknown(Value: string): string;
begin
  if Value = '' then
    ValueOrUnknown := Unknown
  else
    ValueOrUnknown := Value;
end;

procedure SerialInit;
begin
  Port[Com1InterruptEnable] := 0;
  Port[Com1LineControl] := Com1LineControlDlab;
  Port[Com1Data] := BaudDivisor38400;
  Port[Com1InterruptEnable] := 0;
  Port[Com1LineControl] := Com1LineControl8N1;
  Port[Com1FifoControl] := Com1FifoEnableClear;
  Port[Com1ModemControl] := Com1ModemControlReady;
end;

procedure SerialWriteChar(Value: Char);
var
  Delay: Integer;
begin
  while (Port[Com1LineStatus] and Com1LineStatusTransmitterReady) = 0 do
    ;
  Port[Com1Data] := Ord(Value);
  for Delay := 1 to 1000 do
    ;
end;

procedure SerialWriteString(const Value: string);
var
  I: Integer;
begin
  for I := 1 to Length(Value) do
    SerialWriteChar(Value[I]);
end;

procedure SerialWriteLine(const Value: string);
begin
  SerialWriteString(Value);
  SerialWriteChar(#13);
  SerialWriteChar(#10);
end;

function SerialReadChar: Char;
begin
  while (Port[Com1LineStatus] and Com1LineStatusDataReady) = 0 do
    ;
  SerialReadChar := Char(Port[Com1Data]);
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

function ReadLines(Name: string; var Lines: DropLines): boolean;
var
  F: file;
  Buffer: FileBuffer;
  { Free Pascal's BlockRead result parameter is wider than Turbo Pascal's Word. }
  Count: LongInt;
  I: Integer;
  LineNumber: Integer;
  Ch: Char;
  LastWasCr: Boolean;
begin
  ReadLines := false;
  for I := 1 to 10 do
    Lines[I] := '';

  Assign(F, Name);
  {$I-}
  Reset(F, 1);
  {$I+}
  if IOResult <> 0 then
    Exit;

  LineNumber := 1;
  LastWasCr := false;
  repeat
    Count := 0;
    {$I-}
    BlockRead(F, Buffer, SizeOf(Buffer), Count);
    {$I+}
    if IOResult <> 0 then
      Break;

    for I := 1 to Count do
    begin
      Ch := Char(Buffer[I]);
      if (Ch = #13) or (Ch = #10) then
      begin
        if not ((Ch = #10) and LastWasCr) then
        begin
          if LineNumber < 10 then
            LineNumber := LineNumber + 1;
        end;
        LastWasCr := Ch = #13;
      end
      else
      begin
        LastWasCr := false;
        if Length(Lines[LineNumber]) < 120 then
          Lines[LineNumber] := Lines[LineNumber] + Ch;
      end;
    end;
  until Count = 0;

  Close(F);
  ReadLines := true;
end;

procedure ReadNodeFile;
var
  Lines: DropLines;
  Line: string;
begin
  if not ReadLines(NodeFile, Lines) then
    Exit;

  Line := Lines[1];
  if Copy(Line, 1, 5) = 'node=' then
    NodeNumber := ValueOrUnknown(Copy(Line, 6, Length(Line) - 5));
end;

procedure ParseDorInfo(var Lines: DropLines);
begin
  ActiveDropFile := DropDorInfo;
  BoardName := ValueOrUnknown(Lines[1]);
  SysopName := ValueOrUnknown(Lines[2]);
  CallerName := ValueOrUnknown(Lines[6] + ' ' + Lines[7]);
  CallerAlias := CallerName;
  CallerLocation := ValueOrUnknown(Lines[8]);
  SecurityLevel := ValueOrUnknown(Lines[9]);
  MinutesRemaining := ValueOrUnknown(Lines[10]);
end;

procedure ParseDoorSys(var Lines: DropLines);
begin
  ActiveDropFile := DropDoorSys;
  NodeNumber := ValueOrUnknown(Lines[4]);
  MinutesRemaining := ValueOrUnknown(Lines[5]);
  CallerAlias := ValueOrUnknown(Lines[6]);
  CallerName := ValueOrUnknown(Lines[7]);
  CallerLocation := ValueOrUnknown(Lines[8]);
  SecurityLevel := ValueOrUnknown(Lines[9]);
end;

function LoadDropFile: boolean;
var
  Lines: DropLines;
  Found: Boolean;
begin
  Found := false;
  if ReadLines(DropDorInfo, Lines) then
  begin
    ParseDorInfo(Lines);
    Found := true;
  end
  else if ReadLines(DropDoorSys, Lines) then
  begin
    ParseDoorSys(Lines);
    Found := true;
  end;

  if Found then
    ReadNodeFile;

  { Avoid testing the function name directly; in TP mode that recurses. }
  LoadDropFile := Found;
end;

procedure PrintSummary;
begin
  SerialWriteLine('');
  SerialWriteLine('Drop file: ' + ActiveDropFile);
  SerialWriteLine('Node: ' + NodeNumber);
  if BoardName <> Unknown then
    SerialWriteLine('Board: ' + BoardName);
  if SysopName <> Unknown then
    SerialWriteLine('Sysop: ' + SysopName);
  SerialWriteLine('Caller: ' + CallerName);
  if CallerAlias <> CallerName then
    SerialWriteLine('Alias: ' + CallerAlias);
  SerialWriteLine('Location: ' + CallerLocation);
  SerialWriteLine('Security: ' + SecurityLevel);
  SerialWriteLine('Minutes: ' + MinutesRemaining);
  SerialWriteLine('');
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
    SerialWriteLine('ERROR: report file write failed');
    Halt(3);
  end;

  WriteLn(F, 'Oxide Door Check');
  WriteLn(F, 'drop_file=', ActiveDropFile);
  WriteLn(F, 'node=', NodeNumber);
  WriteLn(F, 'caller=', CallerName);
  WriteLn(F, 'result=report');
  Close(F);
  SerialWriteLine('Report file written');
end;

procedure Prompt;
begin
  SerialWriteString('[I]nfo  [R]eport  [Q]uit: ');
end;

begin
  SerialInit;
  InitSummary;
  SerialWriteLine('Oxide Door Check');

  if not LoadDropFile then
  begin
    SerialWriteLine('ERROR: no supported drop file found');
    Halt(2);
  end;

  PrintSummary;

  repeat
    Prompt;
    Choice := UpperAscii(SerialReadChar);
    while (Choice = #13) or (Choice = #10) do
      Choice := UpperAscii(SerialReadChar);
    SerialWriteChar(Choice);
    SerialWriteChar(#13);
    SerialWriteChar(#10);
    case Choice of
      'I': PrintSummary;
      'R': WriteReport;
      'Q':
        begin
          SerialWriteLine('Returning to OxideBBS');
          Halt(0);
        end;
    end;
  until false;
end.
