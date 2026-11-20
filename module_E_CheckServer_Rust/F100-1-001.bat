@echo off
setlocal EnableExtensions EnableDelayedExpansion

:: =====================================================================
:: Studio Airbook - FAS and EAI Server Inspection System
:: Inspection Code : F100-1-001 (EAI Server Daily Inspection)
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
echo   FAS and EAI Server Inspection Report [F100-1-001] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   Inspection Time : %YYYY%-%MM%-%DD% %HH%:%MIN%:%SS% >> "%TXT_FILE%"
echo   Target Device   : EAI Server (FAS#1 / FAS#2) >> "%TXT_FILE%"
echo   Environment     : Windows Server 2025 (Closed Network) >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo. >> "%TXT_FILE%"

echo ==============================================================================
echo [F100-1-001] EAI Server Inspection Start...
echo Log Directory : %TARGET_DIR%
echo ==============================================================================
echo.

:: =====================================================================
:: [1] HA Program Status Check
:: =====================================================================
echo [*] Checking 1. HA Program Status...

:: 1-1. FAS#1 EverRun Port Check
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#1 EAI Server"
set "V_SEQ=1"
set "V_ITM=Stratus EverRun Console Port"
netstat -ano | findstr ":443" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=EverRun Management Port 443 Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 443 Inactive (Check EverRun Console)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 1-2. FAS#2 LifeKeeper Service Check
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#2 EAI Server"
set "V_SEQ=2"
set "V_ITM=LifeKeeper Service and Active Standby Status"
sc query "LifeKeeper" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=LifeKeeper Service Running"
    set /a OK_CNT+=1
) else (
    tasklist /FI "IMAGENAME eq LifeKeeper.exe" 2>nul | findstr /I "LifeKeeper.exe" >nul
    if !errorlevel! equ 0 (
        set "V_RES=PASS"
        set "V_REM=LifeKeeper Process Active"
        set /a OK_CNT+=1
    ) else (
        set "V_RES=WARN"
        set "V_REM=LifeKeeper Local Service Not Found (Check GUI)"
        set /a WARN_CNT+=1
    )
)
call :RECORD_DATA

:: =====================================================================
:: [2] EAI Interface Status Check
:: =====================================================================
echo [*] Checking 2. EAI Interface Status...

:: 2-1. FAS#1 CLAVO IP (172.21.126.36) Ping
set /a TOTAL_CNT+=1
set "V_CAT=2. EAI Interface Status"
set "V_TGT=FAS#1 EAI Server (CLAVO)"
set "V_SEQ=1"
set "V_ITM=Ping 172.21.126.36 Test"
ping -n 1 -w 1000 172.21.126.36 | findstr "TTL=" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=IP 172.21.126.36 Connection OK"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=IP 172.21.126.36 Connection Failed"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 2-2. FAS#2 EAI Server IP (172.30.26.12) Ping
set /a TOTAL_CNT+=1
set "V_CAT=2. EAI Interface Status"
set "V_TGT=FAS#2 EAI Server"
set "V_SEQ=2"
set "V_ITM=Ping 172.30.26.12 Test"
ping -n 1 -w 1000 172.30.26.12 | findstr "TTL=" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=IP 172.30.26.12 Connection OK"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=IP 172.30.26.12 Connection Failed"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [3] EAI Data Tool Status Check
:: =====================================================================
echo [*] Checking 3. EAI Data Tool Status...

:: 3-1. FAS#1 Interface Data Port 80
set /a TOTAL_CNT+=1
set "V_CAT=3. EAI Data Tool Status"
set "V_TGT=FAS#1 EAI Server (CLAVO)"
set "V_SEQ=1"
set "V_ITM=Interface Port 80 Listening"
netstat -ano | findstr ":80" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Port 80 Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 80 Inactive (Check Web Console)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 3-2. FAS#2 Network Traffic
set /a TOTAL_CNT+=1
set "V_CAT=3. EAI Data Tool Status"
set "V_TGT=FAS#2 EAI Server"
set "V_SEQ=2"
set "V_ITM=Network Adapter Traffic Check"
netstat -e | findstr /I "Bytes" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Network Adapter Traffic Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Network Adapter Status Check Needed"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [4] Database Service Status Check
:: =====================================================================
echo [*] Checking 4. Database Service Status...

:: 4-1. MSSQLSERVER Service Status
set /a TOTAL_CNT+=1
set "V_CAT=4. Database Service Status"
set "V_TGT=FAS#1/#2 Database"
set "V_SEQ=1"
set "V_ITM=MSSQLSERVER Service State"
sc query "MSSQLSERVER" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=MSSQLSERVER Service Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=MSSQLSERVER Service Stopped"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 4-2. SQL Port 1433 Listener Check
set /a TOTAL_CNT+=1
set "V_CAT=4. Database Service Status"
set "V_TGT=FAS#1/#2 Database"
set "V_SEQ=2"
set "V_ITM=MS-SQL Listener Port 1433"
netstat -ano | findstr ":1433" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=MS-SQL Port 1433 Listening"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=Port 1433 Inactive (Connection Fail)"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [5] EAI Monitor and System Event Log Check
:: =====================================================================
echo [*] Checking 5. EAI Monitor and Event Logs...

set /a TOTAL_CNT+=1
set "V_CAT=5. EAI Monitor Status"
set "V_TGT=FAS#1/#2 Common"
set "V_SEQ=1"
set "V_ITM=Application Error Log in Past 1 Hour"
wevtutil qe Application "/q:*[System[(Level=1 or Level=2) and TimeCreated[timediff(@SystemTime) <= 3600000]]]" /c:1 /f:text >nul 2>&1
if %errorlevel% equ 0 (
    set "V_RES=WARN"
    set "V_REM=Error or Warning Detected in Event Log"
    set /a WARN_CNT+=1
) else (
    set "V_RES=PASS"
    set "V_REM=No Errors in Past 1 Hour"
    set /a OK_CNT+=1
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
echo Inspection Completed!
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
echo F100-1-001,Daily,EAI Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0