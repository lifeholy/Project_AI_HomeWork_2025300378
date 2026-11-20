@echo off
setlocal EnableExtensions EnableDelayedExpansion

:: =====================================================================
:: Studio Airbook - FAS and EAI Server Inspection System
:: Inspection Code : F100-2-001 (FAS Server Daily Inspection)
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
echo   FAS Server Inspection Report [F100-2-001] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   Inspection Time : %YYYY%-%MM%-%DD% %HH%:%MIN%:%SS% >> "%TXT_FILE%"
echo   Target Device   : FAS Server (FAS#1 / FAS#2) >> "%TXT_FILE%"
echo   Environment     : Windows Server 2025 (Closed Network) >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo. >> "%TXT_FILE%"

echo ==============================================================================
echo [F100-2-001] FAS Server Inspection Start...
echo Log Directory : %TARGET_DIR%
echo ==============================================================================
echo.

:: =====================================================================
:: [1] FAS Server Program Status Check (FAS#1)
:: =====================================================================
echo [*] Checking 1. FAS Server Program Status (FAS#1)...

:: 1-1. FAS#1 Server IP 172.21.126.31 Ping
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=1"
set "V_ITM=Remote IP 172.21.126.31 Ping Test"
ping -n 1 -w 1000 172.21.126.31 | findstr "TTL=" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=IP 172.21.126.31 Connection OK"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=IP 172.21.126.31 No Response"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 1-2. TCP/IP NetBIOS Helper Service (lmhosts)
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=2"
set "V_ITM=TCP/IP NetBIOS Helper Service State"
sc query "lmhosts" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=NetBIOS Helper Service Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=NetBIOS Helper Service Stopped"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 1-3. DBMS (MS-SQL) Service State
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=3"
set "V_ITM=MSSQLSERVER Service State"
sc query "MSSQLSERVER" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=MSSQLSERVER Service Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=MSSQLSERVER Service Inactive"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 1-4. Desigo CC Service / Process Check
set /a TOTAL_CNT+=1
set "V_CAT=1. FAS Server Program Status"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=4"
set "V_ITM=Desigo CC FDCM Connection Process"
tasklist /FI "IMAGENAME eq Gms.Application.exe" 2>nul | findstr /I "Gms.Application" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Desigo CC Application Process Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Desigo CC Process Not Detected (Check SMC Console)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [2] FF_IF Program Status Check (FAS#1)
:: =====================================================================
echo [*] Checking 2. FF_IF Program Status (FAS#1)...

:: 2-1. DNC-Server Network Port Check
set /a TOTAL_CNT+=1
set "V_CAT=2. FF_IF Program Status"
set "V_TGT=FAS#1 DNC-Server"
set "V_SEQ=1"
set "V_ITM=Private Facility Network Connection"
netstat -ano | findstr ":50000" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=DNC Communication Port Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Check DNC-Server Connection Window"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 2-2. BAC-NET Protocol Port (UDP 47808)
set /a TOTAL_CNT+=1
set "V_CAT=2. FF_IF Program Status"
set "V_TGT=FAS#1 BAC-NET"
set "V_SEQ=2"
set "V_ITM=BACnet Port 47808 Activity"
netstat -ano -p udp | findstr ":47808" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=BACnet UDP Port 47808 Listening"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=BACnet Port 47808 Inactive (Check Agent LED)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [3] HA Manager Display Status Check (FAS#2)
:: =====================================================================
echo [*] Checking 3. HA Manager Display Status (FAS#2)...

:: 3-1. FasServer / HA Process Check
set /a TOTAL_CNT+=1
set "V_CAT=3. HA Manager Display Status"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=1"
set "V_ITM=HA Manager Agent and Server Status"
tasklist /FI "IMAGENAME eq FasServer.exe" 2>nul | findstr /I "FasServer.exe" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=FasServer Process Running"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=FasServer Process Inactive (Check HA Activation)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [4] FAS Server Execution Screen Check (FAS#2)
:: =====================================================================
echo [*] Checking 4. FAS Server Execution Screen (FAS#2)...

:: 4-1. Client Interface Listening Port
set /a TOTAL_CNT+=1
set "V_CAT=4. FAS Server Execution Screen"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=1"
set "V_ITM=Information Client Connection Port"
netstat -ano | findstr ":8080" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Server Information Port Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=CHECK"
    set "V_REM=Check FAS GUI Screen (All-Stop / EAI-Stop Status)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [5] Server and GDS Client Communication Check
:: =====================================================================
echo [*] Checking 5. Server and GDS Client Communication...

:: 5-1. FAS#1 OPC Server Port 135 (DCOM) Check
set /a TOTAL_CNT+=1
set "V_CAT=5. Server and GDS Client Comm"
set "V_TGT=FAS#1 FAS Server"
set "V_SEQ=1"
set "V_ITM=OPC Server DA Communication Port"
netstat -ano | findstr ":135" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=OPC Server DA Port 135 Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 135 Inactive (Check OPC Server Configuration)"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 5-2. FAS#2 Event Count & System Event Logs
set /a TOTAL_CNT+=1
set "V_CAT=5. Server and GDS Client Comm"
set "V_TGT=FAS#2 FAS Server"
set "V_SEQ=2"
set "V_ITM=Event Information Count and Error Logs"
wevtutil qe Application "/q:*[System[(Level=1 or Level=2) and TimeCreated[timediff(@SystemTime) <= 3600000]]]" /c:1 /f:text >nul 2>&1
if %errorlevel% equ 0 (
    set "V_RES=WARN"
    set "V_REM=Recent Application Warning/Error Detected"
    set /a WARN_CNT+=1
) else (
    set "V_RES=PASS"
    set "V_REM=Event Communication Normal (No Errors)"
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
echo F100-2-001,Daily,FAS Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0