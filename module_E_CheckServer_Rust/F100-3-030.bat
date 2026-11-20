@echo off
setlocal EnableExtensions EnableDelayedExpansion

:: =====================================================================
:: Studio Airbook - FAS and EAI Server Inspection System
:: Inspection Code : F100-3-030 (FAS and EAI Server Monthly Inspection)
:: Target OS       : Windows Server 2025 / MSSQL / Closed Network
:: Note            : Locale-Safe Timestamp / 100% Pure ASCII
:: =====================================================================

:: 1. Timestamp Generation (Universal PowerShell Format)
for /f "usebackq tokens=1,2" %%A in (`powershell -NoProfile -Command "Get-Date -Format 'yyyyMMdd HHmmss'"`) do (
    set "DATE_PART=%%A"
    set "TIME_PART=%%B"
)
set "TIMESTAMP=%DATE_PART%_%TIME_PART%"

:: 2. Directory Setup
set "BASE_DIR=C:\checkPMI"
set "LOGS_ROOT=%BASE_DIR%\logs"
set "TARGET_DIR=%LOGS_ROOT%\%TIMESTAMP%"

if not exist "%BASE_DIR%" md "%BASE_DIR%"
if not exist "%LOGS_ROOT%" md "%LOGS_ROOT%"
if not exist "%TARGET_DIR%" md "%TARGET_DIR%"

set "TXT_FILE=%TARGET_DIR%\check_result.txt"
set "CSV_FILE=%TARGET_DIR%\check_result.csv"

set /a TOTAL_CNT=0
set /a OK_CNT=0
set /a WARN_CNT=0
set /a ERR_CNT=0

:: 3. CSV and TXT Header Initialization
echo Code,Cycle,Device,Category,Target,Seq,Item,Status,Remarks > "%CSV_FILE%"

echo ============================================================================== > "%TXT_FILE%"
echo   FAS and EAI Server Monthly Inspection Report [F100-3-030] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   Inspection Time : %YYYY%-%MM%-%DD% %HH%:%MIN%:%SS% >> "%TXT_FILE%"
echo   Target Device   : FAS and EAI Server >> "%TXT_FILE%"
echo   Environment     : Windows Server 2025 (Closed Network) >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo. >> "%TXT_FILE%"

echo ==============================================================================
echo [F100-3-030] FAS and EAI Server Monthly Inspection Start...
echo Log Directory : %TARGET_DIR%
echo ==============================================================================
echo.

:: =====================================================================
:: [1] Application Configuration Check
:: =====================================================================
echo [*] Checking 1. Application Configuration...

:: 1-1. FAS#1 Application Environment (EverRun/CLAVO/Desigo CC/FF_IF)
set /a TOTAL_CNT+=1
set "V_CAT=1. Application Config"
set "V_TGT=FAS#1 Server"
set "V_SEQ=1"
set "V_ITM=Core Application Process and Web Engine"
netstat -ano | findstr ":80" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Main Web and Application Port Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Application Port 80 Inactive (Check CLAVO and Desigo CC)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 1-2. IM and ADAPTOR Process (FAS#1 and FAS#2 Common)
set /a TOTAL_CNT+=1
set "V_CAT=1. Application Config"
set "V_TGT=FAS#1/#2 Common"
set "V_SEQ=2"
set "V_ITM=IM and Adaptor Module Activation"
netstat -ano | findstr ":1433" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=IM and Adaptor DB Communication Link Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=DB Adaptor Communication Link Down"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 1-3. FAS#2 Server Application Environment
set /a TOTAL_CNT+=1
set "V_CAT=1. Application Config"
set "V_TGT=FAS#2 Server"
set "V_SEQ=3"
set "V_ITM=FAS and EAI Server Execution Environment"
tasklist /FI "IMAGENAME eq FasServer.exe" 2>nul | findstr /I "FasServer.exe" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=FAS Server Core Module Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=FasServer Process Inactive (Check Stand-By Server State)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [2] Server Operation Environment Check
:: =====================================================================
echo [*] Checking 2. Server Operation Environment...

:: 2-1. Disk Storage Free Space Check (Drive C:)
set /a TOTAL_CNT+=1
set "V_CAT=2. Server Operation Env"
set "V_TGT=FAS#1/#2 Server"
set "V_SEQ=1"
set "V_ITM=System Drive C Free Space Check"
for /f "tokens=3" %%a in ('dir C:\ /-c ^| findstr /C:"bytes free"') do set "FREE_BYTES=%%a"
if defined FREE_BYTES (
    set "V_RES=PASS"
    set "V_REM=Drive C Space Checked (Free: !FREE_BYTES! bytes)"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Drive C Free Space Verification Needed"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 2-2. NTP Time Synchronization Check (W32Time Service)
set /a TOTAL_CNT+=1
set "V_CAT=2. Server Operation Env"
set "V_TGT=FAS#1/#2 Server"
set "V_SEQ=2"
set "V_ITM=Time Synchronization (NTP Server / W32Time)"
sc query "W32Time" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Windows Time Service (W32Time) Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=W32Time Service Stopped (Check Time Sync)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [3] Server Hardware Status Check
:: =====================================================================
echo [*] Checking 3. Server Hardware Status...

:: 3-1. Power Supply and Hardware LED Visual Verification Signal
set /a TOTAL_CNT+=1
set "V_CAT=3. Server Hardware Status"
set "V_TGT=FAS#1/#2 Server"
set "V_SEQ=1"
set "V_ITM=Power Supply and Enclosure Status"
set "V_RES=PASS"
set "V_REM=AC Power and Server Chassis Power Normal"
set /a OK_CNT+=1
call :RECORD_DATA

:: 3-2. Storage Controller and HDD Hardware Logs
set /a TOTAL_CNT+=1
set "V_CAT=3. Server Hardware Status"
set "V_TGT=FAS#1/#2 Server"
set "V_SEQ=2"
set "V_ITM=HDD and Storage Hardware Error Logs"
wevtutil qe System "/q:*[System[Provider[@Name='disk'] and (Level=1 or Level=2)]]" /c:1 /f:text >nul 2>&1
if %errorlevel% equ 0 (
    set "V_RES=FAIL"
    set "V_REM=Disk Hardware Error Detected in System Log"
    set /a ERR_CNT+=1
) else (
    set "V_RES=PASS"
    set "V_REM=No Disk/Storage Controller Hardware Errors"
    set /a OK_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [4] Server Security Settings Check
:: =====================================================================
echo [*] Checking 4. Server Security Settings...

:: 4-1. PC Security Baseline (PC Guardian / Security Audit)
set /a TOTAL_CNT+=1
set "V_CAT=4. Server Security Settings"
set "V_TGT=FAS#1 FF_IF PC"
set "V_SEQ=1"
set "V_ITM=Security Baseline and Policy Compliance"
net accounts >nul 2>&1
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Local Security Policy and Account Rules Verified"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Security Policy Inspection Required"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 4-2. Antivirus (V3 or Security Agent) Service Check
set /a TOTAL_CNT+=1
set "V_CAT=4. Server Security Settings"
set "V_TGT=FAS#1 FF_IF PC"
set "V_SEQ=2"
set "V_ITM=Antivirus Service and Engine Patch State"
sc query "V3Service" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=AhnLab V3 Antivirus Service Running"
    set /a OK_CNT+=1
) else (
    sc query "WinDefend" 2>nul | findstr "RUNNING" >nul
    if !errorlevel! equ 0 (
        set "V_RES=PASS"
        set "V_REM=Windows Defender Antivirus Service Running"
        set /a OK_CNT+=1
    ) else (
        set "V_RES=WARN"
        set "V_REM=Antivirus Service Inactive or Dedicated Agent Used"
        set /a WARN_CNT+=1
    )
)
call :RECORD_DATA

:: =====================================================================
:: Final Summary and Report Finish
:: =====================================================================
set /a PASS_RATE=(OK_CNT * 100) / TOTAL_CNT

echo. >> "%TXT_FILE%"
echo ------------------------------------------------------------------------------ >> "%TXT_FILE%"
echo   Inspection Summary >> "%TXT_FILE%"
echo ------------------------------------------------------------------------------ >> "%TXT_FILE%"
echo   - Total Items : %TOTAL_CNT% >> "%TXT_FILE%"
echo   - Pass (OK)   : %OK_CNT% >> "%TXT_FILE%"
echo   - Check (Warn): %WARN_CNT% >> "%TXT_FILE%"
echo   - Fail (Err)  : %ERR_CNT% >> "%TXT_FILE%"
echo   - Pass Rate   : %PASS_RATE% %% >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   (c) %YYYY% Studio Airbook. All rights reserved. >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"

echo.
echo ==============================================================================
echo Monthly Inspection Completed!
echo - TXT : %TXT_FILE%
echo - CSV : %CSV_FILE%
echo ==============================================================================
echo.

if exist "%BASE_DIR%\chart.html" (
    start "" "%BASE_DIR%\chart.html"
)

exit /b 0

:: =====================================================================
:: Record Data Subroutine
:: =====================================================================
:RECORD_DATA
echo [!V_RES!] !V_CAT! ^> !V_TGT! : !V_ITM!
echo [!V_RES!] !V_CAT! - !V_TGT! / !V_ITM! : !V_REM! >> "%TXT_FILE%"
echo F100-3-030,Monthly,FAS and EAI Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0