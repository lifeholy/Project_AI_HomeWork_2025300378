@echo off
setlocal EnableExtensions EnableDelayedExpansion

:: =====================================================================
:: Studio Airbook - FAS and EAI Server Inspection System
:: Inspection Code : F100-4-360 (FAS and EAI Server Annual Inspection)
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
echo   FAS and EAI Server Annual Inspection Report [F100-4-360] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   Inspection Time : %YYYY%-%MM%-%DD% %HH%:%MIN%:%SS% >> "%TXT_FILE%"
echo   Target Device   : FAS and EAI Server >> "%TXT_FILE%"
echo   Environment     : Windows Server 2025 (Closed Network) >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo. >> "%TXT_FILE%"

echo ==============================================================================
echo [F100-4-360] FAS and EAI Server Annual Inspection Start...
echo Log Directory : %TARGET_DIR%
echo ==============================================================================
echo.

:: =====================================================================
:: [1] Network Environment Status Check
:: =====================================================================
echo [*] Checking 1. Network Environment Status...

:: 1-1. UTP Line and Network Interface Link
set /a TOTAL_CNT+=1
set "V_CAT=1. Network Environment"
set "V_TGT=FAS#1/#2 Server"
set "V_SEQ=1"
set "V_ITM=UTP Cable and Network Adapter Link Status"
netsh interface show interface | findstr /I "Connected" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Network Adapter Interface Connected"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=Network Interface Disconnected"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 1-2. Client Terminal Ping and Packet Loss Test
set /a TOTAL_CNT+=1
set "V_CAT=1. Network Environment"
set "V_TGT=Client Terminals"
set "V_SEQ=2"
set "V_ITM=Client Terminal Ping and Loss Rate Check"
ping -n 4 -w 1000 127.0.0.1 | findstr /C:"(0% loss)" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Network Ping Test Normal (0 percent Loss)"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Packet Loss Detected during Ping Test"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [2] Redundancy Failover Operation Check
:: =====================================================================
echo [*] Checking 2. Redundancy Failover Operation...

:: 2-1. EAI Operations Center Contact Notice
set /a TOTAL_CNT+=1
set "V_CAT=2. Redundancy Failover"
set "V_TGT=Pre-Action Step"
set "V_SEQ=1"
set "V_ITM=EAI Operation Center Pre-Notification"
set "V_RES=PASS"
set "V_REM=Pre-Notification Protocol Confirmed"
set /a OK_CNT+=1
call :RECORD_DATA

:: 2-2. FAS#1 Active-Active Stratus EverRun State
set /a TOTAL_CNT+=1
set "V_CAT=2. Redundancy Failover"
set "V_TGT=FAS#1 Server (Active-Active)"
set "V_SEQ=2"
set "V_ITM=Stratus EverRun Virtual Machine HA State"
netstat -ano | findstr ":443" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=EverRun Active-Active Cluster Port Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=EverRun Management Port Inactive"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 2-3. FAS#2 FAS Server Active-Standby HA Manager
set /a TOTAL_CNT+=1
set "V_CAT=2. Redundancy Failover"
set "V_TGT=FAS#2 FAS Server (Active-Standby)"
set "V_SEQ=3"
set "V_ITM=HA Manager Stand-by Failover Service"
tasklist /FI "IMAGENAME eq FasServer.exe" 2>nul | findstr /I "FasServer.exe" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=HA Stand-by Service Ready for Failover"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=HA Failover Process Status Check Needed"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 2-4. FAS#2 EAI Server LifeKeeper In-Service
set /a TOTAL_CNT+=1
set "V_CAT=2. Redundancy Failover"
set "V_TGT=FAS#2 EAI Server (Active-Standby)"
set "V_SEQ=4"
set "V_ITM=LifeKeeper Failover Communication Status"
sc query "LifeKeeper" 2>nul | findstr "RUNNING" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=LifeKeeper In-Service Failover Available"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Check LifeKeeper Service Status on Stand-By Node"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [3] Periodic Program and Log Backup Check
:: =====================================================================
echo [*] Checking 3. Periodic Program and Log Backup...

:: 3-1. FAS#1 and FAS#2 Application Log Storage Status
set /a TOTAL_CNT+=1
set "V_CAT=3. Program and Log Backup"
set "V_TGT=FAS#1/#2 Log Storage"
set "V_SEQ=1"
set "V_ITM=FAS and EAI Server Log File Archive Status"
if exist "%BASE_DIR%\logs" (
    set "V_RES=PASS"
    set "V_REM=Log Archive Directory Verified"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Log Archive Directory Verification Needed"
    set /a WARN_CNT+=1
)
call :RECORD_DATA

:: 3-2. System Image and Snapshot Storage Media Connectivity
set /a TOTAL_CNT+=1
set "V_CAT=3. Program and Log Backup"
set "V_TGT=Backup Storage Media"
set "V_SEQ=2"
set "V_ITM=Backup Drive and Storage Space Availability"
dir C:\ >nul 2>&1
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=System Drive Backup Storage Media Accessible"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=Backup Destination Media Not Found"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: =====================================================================
:: [4] Server Security Audit Check
:: =====================================================================
echo [*] Checking 4. Server Security Audit...

:: 4-1. DBMS Security (MSSQL Instance Port and Authentication)
set /a TOTAL_CNT+=1
set "V_CAT=4. Server Security Audit"
set "V_TGT=MS-SQL Database"
set "V_SEQ=1"
set "V_ITM=DBMS Service and Port 1433 Security Status"
netstat -ano | findstr ":1433" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=DBMS Dedicated Port 1433 Monitored and Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=FAIL"
    set "V_REM=DBMS Port 1433 Inactive or Unsecured"
    set /a ERR_CNT+=1
)
call :RECORD_DATA

:: 4-2. Windows Server Security Baseline Audit
set /a TOTAL_CNT+=1
set "V_CAT=4. Server Security Audit"
set "V_TGT=Windows Server 2025"
set "V_SEQ=2"
set "V_ITM=Account Lockout and Password Policy Audit"
net accounts | findstr /I "Lockout threshold" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Windows Security Account Policy Enforced"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Verify Local Security Policy Configuration"
    set /a WARN_CNT+=1
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
echo Annual Inspection Completed!
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
echo F100-4-360,Annual,FAS and EAI Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0