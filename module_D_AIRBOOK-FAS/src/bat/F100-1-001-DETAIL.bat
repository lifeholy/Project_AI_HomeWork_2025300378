@echo off
setlocal EnableExtensions EnableDelayedExpansion
for /f "usebackq tokens=1,2" %%A in (`powershell -NoProfile -Command "Get-Date -Format 'yyyyMMdd HHmmss'"`) do (
    set "DATE_PART=%%A"
    set "TIME_PART=%%B"
)
set "TIMESTAMP=%DATE_PART%_%TIME_PART%"
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
echo Code,Cycle,Device,Category,Target,Seq,Item,Status,Remarks > "%CSV_FILE%"
echo ============================================================================== > "%TXT_FILE%"
echo   EAI Server DETAILED Inspection Report [F100-1-001-DETAIL] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
set /a TOTAL_CNT+=1
set "V_CAT=1. HA Program Status"
set "V_TGT=FAS#1 EAI Server"
set "V_SEQ=1"
set "V_ITM=EverRun Web Management Port (443)"
netstat -ano | findstr ":443" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Port 443 Active"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 443 Inactive"
    set /a WARN_CNT+=1
)
call :RECORD_DATA
set /a TOTAL_CNT+=1
set "V_CAT=2. EAI Interface Status"
set "V_TGT=FAS#1 EAI Server (CLAVO)"
set "V_SEQ=2"
set "V_ITM=CLAVO Web Port 80 Check"
netstat -ano | findstr ":80" >nul
if %errorlevel% equ 0 (
    set "V_RES=PASS"
    set "V_REM=Port 80 Open"
    set /a OK_CNT+=1
) else (
    set "V_RES=WARN"
    set "V_REM=Port 80 Closed"
    set /a WARN_CNT+=1
)
call :RECORD_DATA
set /a PASS_RATE=(OK_CNT * 100) / TOTAL_CNT
echo   - Pass Rate : %PASS_RATE% %% >> "%TXT_FILE%"
echo Detailed EAI Inspection Completed!
exit /b 0
:RECORD_DATA
echo [!V_RES!] !V_CAT! ^> !V_TGT! : !V_ITM!
echo [!V_RES!] !V_CAT! - !V_TGT! / !V_ITM! : !V_REM! >> "%TXT_FILE%"
echo F100-1-001,Daily,EAI Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0
