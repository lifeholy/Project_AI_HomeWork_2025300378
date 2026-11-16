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
echo   FAS Server Inspection Report [F100-2-001] >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
echo   Inspection Time : %TIMESTAMP% >> "%TXT_FILE%"
echo   Target Device   : FAS Server (FAS#1 / FAS#2) >> "%TXT_FILE%"
echo ============================================================================== >> "%TXT_FILE%"
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
set /a PASS_RATE=(OK_CNT * 100) / TOTAL_CNT
echo. >> "%TXT_FILE%"
echo   - Pass Rate : %PASS_RATE% %% >> "%TXT_FILE%"
echo Inspection Completed! Result saved in: %TARGET_DIR%
exit /b 0
:RECORD_DATA
echo [!V_RES!] !V_CAT! ^> !V_TGT! : !V_ITM!
echo [!V_RES!] !V_CAT! - !V_TGT! / !V_ITM! : !V_REM! >> "%TXT_FILE%"
echo F100-2-001,Daily,FAS Server,!V_CAT!,!V_TGT!,!V_SEQ!,!V_ITM!,!V_RES!,!V_REM! >> "%CSV_FILE%"
exit /b 0
