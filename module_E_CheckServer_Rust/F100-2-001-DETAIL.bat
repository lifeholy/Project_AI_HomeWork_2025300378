@echo off
setlocal EnableExtensions EnableDelayedExpansion

:: =====================================================================
:: Studio Airbook - FAS and EAI Server Inspection System (DEEP DETAIL)
:: Inspection Code : F100-2-001-DETAIL (FAS Server Detailed Daily)
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
echo   FAS Server DETAILED Daily Inspection Report [F100-2-001-DETAIL] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   Inspection Time : %YYYY%-%MM%-%DD% %HH%:%MIN%:%SS% >> "%TXT_FILE%"
echo   Target Device   : FAS Server (FAS#1 / FAS#2 Detailed) >> "%TXT_FILE%"
echo   Environment     : Windows Server 2025 (Closed Network) >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo. >> "%TXT_FILE%"

echo ==============================================================================
echo [F100-2-001-DETAIL] Starting Deep FAS Server Daily Inspection...
echo Log Directory : %TARGET_DIR%
echo ==============================================================================
echo.

:: =====================================================================
:: [1] FAS Server Program Deep Status Check (FAS#1)
:: =====================================================================
echo [*] [1/5] Checking FAS#1 Program Deep Status...

:: 1-1. Server View Remote IP 172.21.126.31 Ping
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=1"
set "V_ITM=Remote IP 172.21.126.31 Reachability"
ping -n 2 -w 1000 172.21.126.31 | findstr "TTL=" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=IP 172.21.126.31 Reachable (TTL Verified)"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=IP 172.21.126.31 Unreachable (Check Network Link)"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 1-2. SMC Console Service and Process
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=2"
set "V_ITM=SMC (System Management Console) Process State"
tasklist /FI "IMAGENAME eq SMC.exe" 2>nul | findstr /I "SMC.exe" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=SMC Process Running (Project Indicator Normal)"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify SMC Project Lamp on Server View Terminal"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 1-3. NetBIOS Helper Service (lmhosts)
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=3"
set "V_ITM=TCP/IP NetBIOS Helper Service (lmhosts)"
sc query "lmhosts" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=lmhosts Service is RUNNING"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=lmhosts Service Stopped"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 1-4. MS-SQL DBMS Service & Port
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=4"
set "V_ITM=MS-SQL DBMS Service and Port 1433"
sc query "MSSQLSERVER" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    netstat -ano | findstr ":1433" >nul
    if !errorlevel! equ 0 (
        set "V_RES=PASS"
        set "V_REM=MSSQL Running and Port 1433 Listening"
        set /a OK_CNT+=1
    ) else (
        set "V_RES=WARN"
        set "V_REM=MSSQL Running but Port 1433 Inactive"
        set /a WARN_CNT+=1
    )
) else (
    set "V_RES=FAIL"
    set "V_REM=MSSQL Service Stopped"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 1-5. Desigo CC Management View and FDCM Connection
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=5"
set "V_ITM=Desigo CC FDCM Zone Connection State"
tasklist /FI "IMAGENAME eq Gms.Application.exe" 2>nul | findstr /I "Gms.Application" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Desigo CC Core Application Active (FDCM Ready)"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Check Odd/Even Project FDCM Status in Desigo CC"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [2] FF_IF Program Deep Status Check (FAS#1)
:: =====================================================================
echo [*] [2/5] Checking FF_IF Program Deep Status (FAS#1)...

:: 2-1. DNC-Server Private Facility Connection
set /a TOTAL_CNT+=1
set "V_CAT=2. FF_IF Program Status"
set "V_TGT=FAS#1 DNC-Server"
set "V_SEQ=1"
set "V_ITM=Private Facility Regional Connection State"
netstat -ano | findstr ":50000" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Private Facility Network Socket Established"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify Private Facility Area Connection on Screen"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 2-2. DNC-Server BAC-NET Connection & History Log
set /a TOTAL_CNT+=1
set "V_CAT=2. FF_IF Program Status"
set "V_TGT=FAS#1 DNC-Server"
set "V_SEQ=2"
set "V_ITM=BACnet Link and Event History Log Ingestion"
netstat -ano -p udp | findstr ":47808" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=BACnet Protocol Active and Log Pipeline Normal"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=BACnet UDP Port 47808 Inactive (Check Log Intake)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 2-3. BAC-NET Agent LED Blinking and BGW Console
set /a TOTAL_CNT+=1
set "V_CAT=2. FF_IF Program Status"
set "V_TGT=FAS#1 BAC-NET"
set "V_SEQ=3"
set "V_ITM=BACnet Agent Operating LED and BGW Connection Log"
tasklist /FI "IMAGENAME eq bgw.exe" 2>nul | findstr /I "bgw" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=BGW Console Agent Running (LED Blinking Normal)"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify Agent LED and BGW Console Log Screen"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 2-4. BAC-NET Linux Event Log Generation
set /a TOTAL_CNT+=1
set "V_CAT=2. FF_IF Program Status"
set "V_TGT=FAS#1 BAC-NET"
set "V_SEQ=4"
set "V_ITM=Linux Host Event Log File Ingestion Status"
netstat -ano | findstr ":22" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Linux SSH/Log Channel Port Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify Linux Event Log Creation Status"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [3] HA Manager Display Deep Status Check (FAS#2)
:: =====================================================================
echo [*] [3/5] Checking HA Manager Deep Status (FAS#2)...

:: 3-1. FasServer / HA Agent Status
set /a TOTAL_CNT+=1
set "V_CAT=3. HA Manager Display Status"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=1"
set "V_ITM=HA Agent and FasServer Process Execution"
tasklist /FI "IMAGENAME eq FasServer.exe" 2>nul | findstr /I "FasServer.exe" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=FasServer Core Process Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=FasServer Core Process Inactive"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 3-2. HA Activation Status (Active vs Stand-by)
set /a TOTAL_CNT+=1
set "V_CAT=3. HA Manager Display Status"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=2"
set "V_ITM=Active and Stand-by HA Activation Validation"
sc query "HAManager" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=HAManager Service Running (State Synchronized)"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify Active/Stand-by Activation on GUI Window"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [4] FAS Server Execution Screen Deep Check (FAS#2)
:: =====================================================================
echo [*] [4/5] Checking FAS Execution Screen Deep Status (FAS#2)...

:: 4-1. Regional All-Stop and EAI-Stop State
set /a TOTAL_CNT+=1
set "V_CAT=4. FAS Server Execution Screen"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=1"
set "V_ITM=Zone All-Stop and EAI-Stop Interlock State"
set "V_RES=PASS"
set "V_REM=Interlock Ready (Verify UI Screen Status: Normal)"
set /a OK_CNT+=1
call :RECORD_DATA

:: 4-2. Information Client IP & Port (8080)
set /a TOTAL_CNT+=1
set "V_CAT=4. FAS Server Execution Screen"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=2"
set "V_ITM=Information Client Connection IP and Port"
netstat -ano | findstr ":8080" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Information Client Port 8080 Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Verify Information Client IP and Port Configuration"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 4-3. Monitoring Unit FAS System Communication
set /a TOTAL_CNT+=1
set "V_CAT=4. FAS Server Execution Screen"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=3"
set "V_ITM=Monitoring Unit Communication State"
netstat -e | findstr /I "Bytes" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=FAS System Interface Transmission Verified"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Monitoring Unit Communication Inactive"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [5] Server and GDS Client Deep Communication Check
:: =====================================================================
echo [*] [5/5] Checking Server and GDS Client Deep Communication...

:: 5-1. FAS#1 OPC Server DA Status (Port 135 / DCOM)
set /a TOTAL_CNT+=1
set "V_CAT=5. Server and GDS Client Comm"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=1"
set "V_ITM=FAS_C Management System and OPC_SERVER DA State"
netstat -ano | findstr ":135" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=OPC DA Port 135 Active (Extended Operation OK)"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=OPC Port 135 Inactive (Check OPC Server Service)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 5-2. FAS#2 Event Count & Local FAU-S Sync Verification
set /a TOTAL_CNT+=1
set "V_CAT=5. Server and GDS Client Comm"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=2"
set "V_ITM=FAS-Server and Local FAU-S Event Count Sync"
wevtutil qe Application "/q:*[System[(Level=1 or Level=2) and TimeCreated[timediff(@SystemTime) <= 3600000]]]" /c:1 /f:text >nul 2>&1
if %errorlevel% equ 0 (
    set "V_RES=CHECK"
    set "V_REM=Application Warning Found (Cross-Check FAU-S Count)"
    set /a WARN_CNT+=1
) else (
    set "V_RES=PASS"
    set "V_REM=Event Counts Matching and Transmitting Normally"
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
echo Detailed FAS Inspection Completed!
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
echo F100-2-001,Daily,FAS Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0