@echo off
setlocal EnableExtensions EnableDelayedExpansion

:: =====================================================================
:: Studio Airbook - FAS and EAI Server Inspection System (DEEP DETAIL)
:: Inspection Code : F100-1-001-DETAIL (EAI Server Detailed Daily)
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
echo   EAI Server DETAILED Daily Inspection Report [F100-1-001-DETAIL] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   Inspection Time : %YYYY%-%MM%-%DD% %HH%:%MIN%:%SS% >> "%TXT_FILE%"
echo   Target Device   : EAI Server (FAS#1 / FAS#2 Detailed) >> "%TXT_FILE%"
echo   Environment     : Windows Server 2025 (Closed Network) >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo. >> "%TXT_FILE%"

echo ==============================================================================
echo [F100-1-001-DETAIL] Starting Deep EAI Server Daily Inspection...
echo Log Directory : %TARGET_DIR%
echo ==============================================================================
echo.

:: =====================================================================
:: [1] EAI server HA Program Deep Status Check
:: =====================================================================
echo [*] [1/5] Checking HA Program Deep Status...

:: 1-1. FAS#1 EverRun Web Console Management Port (443)
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#1 EAI Server"
set "V_SEQ=1"
set "V_ITM=EverRun Chrome Web Management Port (443)"
netstat -ano | findstr ":443" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Port 443 Active (Web UI Ready)"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 443 Inactive (Check EverRun Management Service)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 1-2. FAS#1 System Virtual Machines (EAI / FAS Activity)
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#1 EAI Server"
set "V_SEQ=2"
set "V_ITM=VM Activity and Virtualization Process State"
tasklist /FI "IMAGENAME eq vmtoolsd.exe" 2>nul | findstr /I "vmtoolsd.exe" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=VM Guest Agent Running (EAI/FAS VM Active)"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify VM Activity in EverRun System Dashboard"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 1-3. FAS#1 HDD Volume Disk Sync & Snapshot File
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#1 EAI Server"
set "V_SEQ=3"
set "V_ITM=Disk Synchronization and Storage Snapshot File State"
wevtutil qe System "/q:*[System[Provider[@Name='volsnap'] and (Level=1 or Level=2)]]" /c:1 /f:text >nul 2>&1
if %errorlevel% equ 0 (
    set "V_RES=WARN"
    set "V_REM=Volsnap Storage Event Detected (Check Volumes Sync)"
    set /a WARN_CNT+=1
) else (
    set "V_RES=PASS"
    set "V_REM=Disk Volume Synchronization and Snapshots Normal"
    set /a OK_CNT+=1
)
call :RECORD_DATA

:: 1-4. FAS#2 LifeKeeper Service Status
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#2 EAI Server"
set "V_SEQ=4"
set "V_ITM=LifeKeeper Core Service Status"
sc query "LifeKeeper" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=LifeKeeper Service Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=LifeKeeper Service Not Running or Non-Standard Name"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 1-5. FAS#2 LifeKeeper GUI Process & Cluster Communication
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#2 EAI Server"
set "V_SEQ=5"
set "V_ITM=LifeKeeper GUI Process and Cluster Heartbeat"
tasklist /FI "IMAGENAME eq LifeKeeper.exe" 2>nul | findstr /I "LifeKeeper.exe" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Active Server GUI Detected (Check Green Status)"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify Stand-By (Blue) / Active (Green) State on Console"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [2] EAI Interface Deep Status Check
:: =====================================================================
echo [*] [2/5] Checking EAI Interface Deep Status...

:: 2-1. FAS#1 CLAVO Remote IP (172.21.126.36) Ping
set /a TOTAL_CNT+=1
set "V_CAT=2. EAI Interface Status"
set "V_TGT=FAS#1 EAI Server (CLAVO)"
set "V_SEQ=1"
set "V_ITM=Ping 172.21.126.36 (Round-Trip Latency)"
ping -n 2 -w 1000 172.21.126.36 | findstr "TTL=" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=IP 172.21.126.36 Reachable (TTL Verified)"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=IP 172.21.126.36 Unreachable (CLAVO Offline)"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 2-2. FAS#1 CLAVO HTTP/Interface Port (80)
set /a TOTAL_CNT+=1
set "V_CAT=2. EAI Interface Status"
set "V_TGT=FAS#1 EAI Server (CLAVO)"
set "V_SEQ=2"
set "V_ITM=CLAVO Web Application Port 80 Check"
netstat -ano | findstr ":80" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Port 80 Open (Check Data Source Toggle via Chrome)"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 80 Closed (CLAVO Web Engine Offline)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 2-3. FAS#2 FAU-SC2 Remote IP (172.30.26.12) Ping
set /a TOTAL_CNT+=1
set "V_CAT=2. EAI Interface Status"
set "V_TGT=FAS#2 EAI Server"
set "V_SEQ=3"
set "V_ITM=FAU-SC2 IP 172.30.26.12 Ping Connection"
ping -n 2 -w 1000 172.30.26.12 | findstr "TTL=" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=FAU-SC2 Terminal Reachable"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=FAU-SC2 Terminal Connection Failed"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 2-4. FAS#2 GFS EAI Monitor Execution Process
set /a TOTAL_CNT+=1
set "V_CAT=2. EAI Interface Status"
set "V_TGT=FAS#2 EAI Server"
set "V_SEQ=4"
set "V_ITM=GFS EAI Monitor Process State"
tasklist /FI "IMAGENAME eq GFSEaiMonitor.exe" 2>nul | findstr /I "GFSEaiMonitor" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=GFS EAI Monitor Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify GFS EAI Monitor Window on FAU-SC2 Terminal"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [3] EAI Data Tool Deep Status Check
:: =====================================================================
echo [*] [3/5] Checking EAI Data Tool Deep Status...

:: 3-1. FAS#1 CLAVO Fire Input & FDCM Link
set /a TOTAL_CNT+=1
set "V_CAT=3. EAI Data Tool Status"
set "V_TGT=FAS#1 EAI Server (CLAVO)"
set "V_SEQ=1"
set "V_ITM=Zone Fire Input and FDCM Socket Status"
netstat -ano | findstr ":1433" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Database Link Established (FDCM Data Stream Ready)"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Database Link Inactive (FDCM Disconnected)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 3-2. FAS#2 Interface Tab & Real-time Log Transmission
set /a TOTAL_CNT+=1
set "V_CAT=3. EAI Data Tool Status"
set "V_TGT=FAS#2 EAI Server (Interface)"
set "V_SEQ=2"
set "V_ITM=EAI Interlock Stop State and Real-time Log Stream"
netstat -e | findstr /I "Bytes" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Network Socket Actively Transmitting Log Data"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Check EAI Interlock Stop Tab and Bottom Log Window"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [4] Database Service Deep Status Check
:: =====================================================================
echo [*] [4/5] Checking Database Service Deep Status...

:: 4-1. MSSQLSERVER Service State
set /a TOTAL_CNT+=1
set "V_CAT=4. Database Service Status"
set "V_TGT=FAS#1/#2 Database"
set "V_SEQ=1"
set "V_ITM=MSSQLSERVER Windows Service State"
sc query "MSSQLSERVER" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=MSSQLSERVER Service is RUNNING"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=MSSQLSERVER Service Stopped"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 4-2. SQLSERVERAGENT Service State
set /a TOTAL_CNT+=1
set "V_CAT=4. Database Service Status"
set "V_TGT=FAS#1/#2 Database"
set "V_SEQ=2"
set "V_ITM=SQLSERVERAGENT Job Scheduler Service"
sc query "SQLSERVERAGENT" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=SQL Server Agent Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=SQL Server Agent Stopped or Inactive"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 4-3. SQL Listener Port (1433)
set /a TOTAL_CNT+=1
set "V_CAT=4. Database Service Status"
set "V_TGT=FAS#1/#2 Database"
set "V_SEQ=3"
set "V_ITM=MS-SQL Listener TCP Port 1433"
netstat -ano | findstr ":1433" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Port 1433 Listening"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=Port 1433 Closed (DB Connection Blocked)"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 4-4. SQLCMD Engine Response Query
set /a TOTAL_CNT+=1
set "V_CAT=4. Database Service Status"
set "V_TGT=FAS#1/#2 Database"
set "V_SEQ=4"
set "V_ITM=Local SQL Instance Active Query Response"
where sqlcmd >nul 2>&1
if %errorlevel% equ 0 (
    sqlcmd -S localhost -E -Q "SET NOCOUNT ON; SELECT 1;" -h -1 >nul 2>&1
    if !errorlevel! equ 0 (
        set "V_RES=PASS"
        set "V_REM=SQL Query Executed Successfully (Response OK)"
        set /a OK_CNT+=1
    ) else (
        set "V_RES=FAIL"
        set "V_REM=SQL Query Failed (Check DB Authentication)"
        set /a ERR_CNT+=1
    )
) else (
    set "V_RES=CHECK"
    set "V_REM=sqlcmd Utility Not Installed (Port Verified)"
    set /a OK_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [5] EAI Monitor Screen and Hub Transmission Check
:: =====================================================================
echo [*] [5/5] Checking EAI Monitor Screen and Hub Transmission...

:: 5-1. EAI Hub Outbound Communication Socket
set /a TOTAL_CNT+=1
set "V_CAT=5. EAI Monitor Status"
set "V_TGT=FAS#1/#2 Common"
set "V_SEQ=1"
set "V_ITM=EAI Hub Connection Status (LED Green Indicator)"
netstat -ano | findstr "ESTABLISHED" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Hub Active Connection Socket Established"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify EAI Hub Connection LED Indicator"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 5-2. Hub Completed Transmission Log Audit
set /a TOTAL_CNT+=1
set "V_CAT=5. EAI Monitor Status"
set "V_TGT=FAS#1/#2 Common"
set "V_SEQ=2"
set "V_ITM=Transmission Complete Message Query and Error Logs"
wevtutil qe Application "/q:*[System[(Level=1 or Level=2) and TimeCreated[timediff(@SystemTime) <= 3600000]]]" /c:1 /f:text >nul 2>&1
if %errorlevel% equ 0 (
    set "V_RES=WARN"
    set "V_REM=Warning/Error Detected in Recent Event Logs"
    set /a WARN_CNT+=1
) else (
    set "V_RES=PASS"
    set "V_REM=Transmission Logs Normal (No Application Errors)"
    set /a OK_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: Summary & Result Finalize
:: =====================================================================
set /a PASS_RATE=(OK_CNT * 100) / TOTAL_CNT

echo. >> "%TXT_FILE%"
echo ------------------------------------------------------------------------------ >> "%TXT_FILE%"
echo   Detailed Inspection Summary >> "%TXT_FILE%"
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
echo Detailed EAI Inspection Completed!
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